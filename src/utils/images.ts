import { convertFileSrc } from "@tauri-apps/api/core";

// Bumped after media files change place (e.g. cover <-> screenshot swap): the file names stay
// the same, so without it the webview would keep showing the cached old image
let mediaVersion = 0;
export function bumpMediaVersion() {
  mediaVersion++;
}

/** Absolute path on any OS: "/Users/...", "C:\\..." / "C:/..." or a network path "\\\\server\\..." */
export function isAbsolutePath(path: string): boolean {
  return path.startsWith("/") || /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith("\\\\");
}

/** Absolute path of a gamelist media value ("./images/x.png" is relative to the system folder) */
export function absoluteMediaPath(mediaPath: string, systemPath: string): string {
  if (isAbsolutePath(mediaPath)) return mediaPath;
  const relative = mediaPath.startsWith("./") ? mediaPath.slice(2) : mediaPath;
  return `${systemPath}/${relative}`;
}

function toMediaUrl(filePath: string): string {
  // Our "media" protocol (registered in Rust). convertFileSrc builds the right URL for each OS:
  // media://localhost/<path> on macOS/Linux and http://media.localhost/<path> on Windows
  // (WebView2 doesn't load custom schemes as such), with the whole path encoded.
  // The query is ignored by the Rust handler (it only reads the path).
  return `${convertFileSrc(filePath, "media")}${mediaVersion ? `?v=${mediaVersion}` : ""}`;
}

export function resolveImageSrc(
  imagePath: string | undefined,
  systemPath: string
): string | undefined {
  if (!imagePath) return undefined;

  return toMediaUrl(absoluteMediaPath(imagePath, systemPath));
}
