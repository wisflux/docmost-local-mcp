import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { helperTargets } from "./helper-targets.mjs";

const rootDir = new URL("../", import.meta.url).pathname;
const rootPackageJson = JSON.parse(await readFile(join(rootDir, "package.json"), "utf8"));

for (const target of helperTargets) {
  const packageDir = join(rootDir, "packages", target.packageDir);
  await mkdir(join(packageDir, "bin"), { recursive: true });

  const packageJson = {
    name: target.packageName,
    version: rootPackageJson.version,
    description: target.description,
    license: rootPackageJson.license,
    os: [target.platform],
    cpu: [target.arch],
    files: ["bin"],
    bin: {
      "docmost-auth-helper": `./bin/${target.binaryName}`,
    },
  };

  await writeFile(join(packageDir, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);
}
