import type { ReactNode } from "react";
import { renderHook, act, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, it, expect, vi, beforeEach, afterAll } from "vitest";
import { http, HttpResponse } from "msw";
import type { SkillsResponse } from "@/lib/api/skills";
import {
  useAllSkills,
  useAddSkillRepo,
  useInstallSkill,
  useRemoveSkillRepo,
  useSkillRepos,
  useUninstallSkill,
} from "@/hooks/useSkills";
import { server } from "../msw/server";
import { getSkillsState, getSkillReposState } from "../msw/state";

interface WrapperProps {
  children: ReactNode;
}

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });

  const wrapper = ({ children }: WrapperProps) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper, queryClient };
}

const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

describe("useSkills hooks", () => {
  beforeEach(() => {
    consoleErrorSpy.mockClear();
  });

  afterAll(() => {
    consoleErrorSpy.mockRestore();
  });

  it("fetches skills list", async () => {
    const expected = getSkillsState();
    const { wrapper } = createWrapper();

    const { result } = renderHook(() => useAllSkills(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(expected);
  });

  it("fetches skills list for a specific app", async () => {
    const requestedApps: Array<string | undefined> = [];
    server.use(
      http.post("http://tauri.local/get_skills", async ({ request }) => {
        const body = (await request.json()) as { app?: string };
        requestedApps.push(body.app);
        return HttpResponse.json(getSkillsState());
      }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useAllSkills("codex"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(requestedApps).toContain("codex");
  });

  it("fetches skill repositories", async () => {
    const expected = getSkillReposState();
    const { wrapper } = createWrapper();

    const { result } = renderHook(() => useSkillRepos(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(expected);
  });

  it("adds a skill repo and invalidates repos and skills queries", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const repo = {
      owner: "me",
      name: "new-skill-repo",
      branch: "main",
      enabled: true,
    };

    const reposQuery = renderHook(() => useSkillRepos(), { wrapper });
    await waitFor(() => expect(reposQuery.result.current.isSuccess).toBe(true));

    const { result } = renderHook(() => useAddSkillRepo(), { wrapper });

    await act(async () => {
      await result.current.mutateAsync(repo);
    });

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["skills", "repos"] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["skills", "all"] });

    await act(async () => {
      await queryClient.refetchQueries({ queryKey: ["skills", "repos"] });
    });

    const updated = queryClient.getQueryData<ReturnType<typeof getSkillReposState>>([
      "skills",
      "repos",
    ]);
    expect(updated?.some((item) => item.name === repo.name)).toBe(true);
  });

  it("removes a skill repo and invalidates repos and skills queries", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const repo = getSkillReposState()[0];

    const reposQuery = renderHook(() => useSkillRepos(), { wrapper });
    await waitFor(() => expect(reposQuery.result.current.isSuccess).toBe(true));

    const { result } = renderHook(() => useRemoveSkillRepo(), { wrapper });

    await act(async () => {
      await result.current.mutateAsync({
        owner: repo.owner,
        name: repo.name,
      });
    });

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["skills", "repos"] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["skills", "all"] });

    await act(async () => {
      await queryClient.refetchQueries({ queryKey: ["skills", "repos"] });
    });

    const updated = queryClient.getQueryData<ReturnType<typeof getSkillReposState>>([
      "skills",
      "repos",
    ]);
    expect(updated?.some((item) => item.name === repo.name)).toBe(false);
  });

  it("installs a skill and invalidates the skills query", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const directory = "/skills/notes";

    const skillsQuery = renderHook(() => useAllSkills(), { wrapper });
    await waitFor(() => expect(skillsQuery.result.current.isSuccess).toBe(true));

    const { result } = renderHook(() => useInstallSkill(), { wrapper });

    await act(async () => {
      await result.current.mutateAsync(directory);
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["skills", "all", "claude"],
    });

    await act(async () => {
      await queryClient.refetchQueries({ queryKey: ["skills", "all", "claude"] });
    });

    const updated = queryClient.getQueryData<SkillsResponse>([
      "skills",
      "all",
      "claude",
    ]);
    expect(updated?.skills?.find((skill) => skill.directory === directory)?.installed).toBe(true);
  });

  it("installs a skill with force flag", async () => {
    const { wrapper, queryClient } = createWrapper();
    const directory = "/skills/notes";

    const skillsQuery = renderHook(() => useAllSkills(), { wrapper });
    await waitFor(() => expect(skillsQuery.result.current.isSuccess).toBe(true));

    const { result } = renderHook(() => useInstallSkill(), { wrapper });

    await act(async () => {
      await result.current.mutateAsync({ directory, force: true });
    });

    await act(async () => {
      await queryClient.refetchQueries({ queryKey: ["skills", "all", "claude"] });
    });

    const updated = queryClient.getQueryData<SkillsResponse>([
      "skills",
      "all",
      "claude",
    ]);
    expect(updated?.skills?.find((skill) => skill.directory === directory)?.installed).toBe(true);
  });

  it("installs a skill for a specific app and forwards force/app payload", async () => {
    const payloads: Array<{ directory?: string; force?: boolean; app?: string }> = [];
    server.use(
      http.post("http://tauri.local/install_skill", async ({ request }) => {
        const body = (await request.json()) as {
          directory?: string;
          force?: boolean;
          app?: string;
        };
        payloads.push(body);
        return HttpResponse.json(true);
      }),
    );

    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const { result } = renderHook(() => useInstallSkill("gemini"), { wrapper });

    await act(async () => {
      await result.current.mutateAsync({ directory: "/skills/notes", force: true });
    });

    expect(payloads[0]).toMatchObject({
      directory: "/skills/notes",
      force: true,
      app: "gemini",
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["skills", "all", "gemini"],
    });
  });

  it("uninstalls a skill and invalidates the skills query", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const directory = "/skills/terminal";

    const skillsQuery = renderHook(() => useAllSkills(), { wrapper });
    await waitFor(() => expect(skillsQuery.result.current.isSuccess).toBe(true));

    const { result } = renderHook(() => useUninstallSkill(), { wrapper });

    await act(async () => {
      await result.current.mutateAsync(directory);
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["skills", "all", "claude"],
    });

    await act(async () => {
      await queryClient.refetchQueries({ queryKey: ["skills", "all", "claude"] });
    });

    const updated = queryClient.getQueryData<SkillsResponse>([
      "skills",
      "all",
      "claude",
    ]);
    expect(updated?.skills?.find((skill) => skill.directory === directory)?.installed).toBe(false);
  });

  it("surfaces errors from install mutation", async () => {
    server.use(
      http.post("http://tauri.local/install_skill", () =>
        HttpResponse.json({ message: "install failed" }, { status: 500 }),
      ),
    );
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useInstallSkill(), { wrapper });

    await expect(
      act(async () => {
        await result.current.mutateAsync("/skills/unknown");
      }),
    ).rejects.toThrow(/install failed/);
    expect(invalidateSpy).not.toHaveBeenCalled();
  });
});
