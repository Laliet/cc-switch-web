/// <reference types="vite/client" />

declare global {
  interface ImportMetaEnv {
    readonly VITE_MODE?: string;
    readonly VITE_APP_VERSION?: string;
  }
}

export {};
