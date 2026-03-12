export const helperTargets = [
  { platform: "darwin", arch: "arm64", binaryName: "docmost-auth-helper" },
  { platform: "darwin", arch: "x64", binaryName: "docmost-auth-helper" },
  { platform: "linux", arch: "arm64", binaryName: "docmost-auth-helper" },
  { platform: "linux", arch: "x64", binaryName: "docmost-auth-helper" },
  { platform: "win32", arch: "arm64", binaryName: "docmost-auth-helper.exe" },
  { platform: "win32", arch: "x64", binaryName: "docmost-auth-helper.exe" },
];

export function getHelperTarget(platform, arch) {
  return helperTargets.find((t) => t.platform === platform && t.arch === arch) ?? null;
}

export function helperDir(target) {
  return `${target.platform}-${target.arch}`;
}
