function toMediaUrl(filePath: string): string {
  // Use our custom "media://" protocol registered in Rust
  // Keep the path as-is (including leading /) so Rust receives the full absolute path
  const encoded = filePath
    .split("/")
    .map((seg) => (seg ? encodeURIComponent(seg) : ""))
    .join("/");
  return `media://localhost${encoded}`;
}

export function resolveImageSrc(
  imagePath: string | undefined,
  systemPath: string
): string | undefined {
  if (!imagePath) return undefined;

  // Absolute path
  if (imagePath.startsWith("/")) {
    return toMediaUrl(imagePath);
  }

  // Relative path (./images/foo.png)
  const relative = imagePath.startsWith("./")
    ? imagePath.slice(2)
    : imagePath;

  return toMediaUrl(`${systemPath}/${relative}`);
}
