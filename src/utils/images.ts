// Bumped after media files change place (e.g. cover <-> screenshot swap): the file names stay
// the same, so without it the webview would keep showing the cached old image
let mediaVersion = 0;
export function bumpMediaVersion() {
  mediaVersion++;
}

function toMediaUrl(filePath: string): string {
  // Use our custom "media://" protocol registered in Rust
  // Keep the path as-is (including leading /) so Rust receives the full absolute path
  const encoded = filePath
    .split("/")
    .map((seg) => (seg ? encodeURIComponent(seg) : ""))
    .join("/");
  // The query is ignored by the Rust protocol handler (it only reads the path)
  return `media://localhost${encoded}${mediaVersion ? `?v=${mediaVersion}` : ""}`;
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
