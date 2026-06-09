function formatUuidV4FromBytes(bytes: Uint8Array): string {
  // Per RFC 4122 v4:
  // - Set version to 4 (0b0100)
  // - Set variant to RFC 4122 (0b10xx)
  const bytesCopy = new Uint8Array(bytes);
  bytesCopy[6] = (bytesCopy[6] & 0x0f) | 0x40;
  bytesCopy[8] = (bytesCopy[8] & 0x3f) | 0x80;

  const hex = Array.from(bytesCopy, (byte) =>
    byte.toString(16).padStart(2, "0"),
  );

  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10, 16).join(""),
  ].join("-");
}

function generateMathRandomUuidV4(): string {
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (char) => {
    const randomNibble = Math.floor(Math.random() * 16);
    const value = char === "x" ? randomNibble : (randomNibble & 0x3) | 0x8;
    return value.toString(16);
  });
}

export function generateUUID(): string {
  const cryptoObject = globalThis.crypto as Crypto | undefined;

  const randomUUID = (cryptoObject as any)?.randomUUID;
  if (typeof randomUUID === "function") {
    return randomUUID.call(cryptoObject);
  }

  const getRandomValues = cryptoObject?.getRandomValues;
  if (typeof getRandomValues === "function") {
    const bytes = new Uint8Array(16);
    getRandomValues.call(cryptoObject, bytes);
    return formatUuidV4FromBytes(bytes);
  }

  return generateMathRandomUuidV4();
}

export default generateUUID;
