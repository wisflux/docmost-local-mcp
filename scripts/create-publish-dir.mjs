import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { buildOptionalDependencies } from "./helper-targets.mjs";

const rootDir = new URL("../", import.meta.url).pathname;
const publishDir = join(rootDir, ".publish", "main");
const rootPackagePath = join(rootDir, "package.json");
const rootPackageJson = JSON.parse(await readFile(rootPackagePath, "utf8"));

await rm(publishDir, { recursive: true, force: true });
await mkdir(publishDir, { recursive: true });

const publishPackageJson = {
  ...rootPackageJson,
  optionalDependencies: buildOptionalDependencies(rootPackageJson.version),
};

await writeFile(join(publishDir, "package.json"), `${JSON.stringify(publishPackageJson, null, 2)}\n`);
await cp(join(rootDir, "README.md"), join(publishDir, "README.md"));
await cp(join(rootDir, "LICENSE"), join(publishDir, "LICENSE"));
await cp(join(rootDir, "dist"), join(publishDir, "dist"), { recursive: true });
