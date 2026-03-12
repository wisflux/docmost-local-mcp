export const helperTargets = [
  {
    platform: "darwin",
    arch: "arm64",
    packageDir: "auth-helper-darwin-arm64",
    packageName: "@docmost-local-mcp/auth-helper-darwin-arm64",
    binaryName: "docmost-auth-helper",
    description: "Native auth helper for docmost-local-mcp on macOS arm64",
  },
  {
    platform: "darwin",
    arch: "x64",
    packageDir: "auth-helper-darwin-x64",
    packageName: "@docmost-local-mcp/auth-helper-darwin-x64",
    binaryName: "docmost-auth-helper",
    description: "Native auth helper for docmost-local-mcp on macOS x64",
  },
  {
    platform: "linux",
    arch: "arm64",
    packageDir: "auth-helper-linux-arm64",
    packageName: "@docmost-local-mcp/auth-helper-linux-arm64",
    binaryName: "docmost-auth-helper",
    description: "Native auth helper for docmost-local-mcp on Linux arm64",
  },
  {
    platform: "linux",
    arch: "x64",
    packageDir: "auth-helper-linux-x64",
    packageName: "@docmost-local-mcp/auth-helper-linux-x64",
    binaryName: "docmost-auth-helper",
    description: "Native auth helper for docmost-local-mcp on Linux x64",
  },
  {
    platform: "win32",
    arch: "arm64",
    packageDir: "auth-helper-win32-arm64",
    packageName: "@docmost-local-mcp/auth-helper-win32-arm64",
    binaryName: "docmost-auth-helper.exe",
    description: "Native auth helper for docmost-local-mcp on Windows arm64",
  },
  {
    platform: "win32",
    arch: "x64",
    packageDir: "auth-helper-win32-x64",
    packageName: "@docmost-local-mcp/auth-helper-win32-x64",
    binaryName: "docmost-auth-helper.exe",
    description: "Native auth helper for docmost-local-mcp on Windows x64",
  },
];

export function getHelperTarget(platform, arch) {
  return helperTargets.find((target) => target.platform === platform && target.arch === arch) ?? null;
}

export function buildOptionalDependencies(version) {
  return Object.fromEntries(helperTargets.map((target) => [target.packageName, version]));
}
