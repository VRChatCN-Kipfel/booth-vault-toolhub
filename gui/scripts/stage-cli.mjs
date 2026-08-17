import { cpSync, existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "..", "..");
const isWin = process.platform === "win32";
const names = isWin
  ? ["booth.exe", "booth-mcp.exe", "booth-shell.exe"]
  : ["booth", "booth-mcp"];

const candidates = [];
const collectRelease = (dir) => {
  const release = join(dir, "release");
  if (existsSync(release)) candidates.push(release);
};
collectRelease(join(root, "target"));
if (existsSync(join(root, "target"))) {
  for (const entry of readdirSync(join(root, "target"), { withFileTypes: true })) {
    if (entry.isDirectory()) collectRelease(join(root, "target", entry.name));
  }
}

const picked = new Map();
for (const dir of candidates) {
  for (const name of names) {
    const path = join(dir, name);
    if (!existsSync(path)) continue;
    const mtime = statSync(path).mtimeMs;
    if (!picked.has(name) || mtime > picked.get(name)[1]) {
      picked.set(name, [path, mtime]);
    }
  }
}

const missing = names.filter((name) => !picked.has(name));
if (missing.length > 0) {
  console.error(`[stage-cli] 缺少 CLI 二进制：${missing.join(", ")}`);
  console.error("[stage-cli] 请先执行 cargo build --release --workspace");
  process.exit(1);
}

const boothKey = isWin ? "booth.exe" : "booth";
const mcpKey = isWin ? "booth-mcp.exe" : "booth-mcp";
const boothSrc = picked.get(boothKey)[0];
const boothMcpSrc = picked.get(mcpKey)[0];
const boothShellSrc = isWin ? picked.get("booth-shell.exe")[0] : "";
const projectOut = dirname(boothSrc);

if (!isWin) {
  console.log("[stage-cli] 非 Windows：跳过 MSI/NSIS 片段，CLI 已定位");
  console.log(`[stage-cli] booth -> ${boothSrc}`);
  console.log(`[stage-cli] booth-mcp -> ${boothMcpSrc}`);
  process.exit(0);
}

const xmlEsc = (s) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const wxs = `<?if $(sys.BUILDARCH)="x86"?>
    <?define Win64 = "no" ?>
<?elseif $(sys.BUILDARCH)="x64"?>
    <?define Win64 = "yes" ?>
<?elseif $(sys.BUILDARCH)="arm64"?>
    <?define Win64 = "yes" ?>
<?else?>
    <?error Unsupported value of sys.BUILDARCH=$(sys.BUILDARCH)?>
<?endif?>

<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <DirectoryRef Id="INSTALLDIR">
            <Component Id="booth.exe" Guid="*" Win64="$(var.Win64)">
                <File Id="booth_exe_file" Source="${xmlEsc(boothSrc)}" KeyPath="yes"/>
            </Component>
            <Component Id="booth_mcp.exe" Guid="*" Win64="$(var.Win64)">
                <File Id="booth_mcp_exe_file" Source="${xmlEsc(boothMcpSrc)}" KeyPath="yes"/>
            </Component>
            <Component Id="booth_shell.exe" Guid="*" Win64="$(var.Win64)">
                <File Id="booth_shell_exe_file" Source="${xmlEsc(boothShellSrc)}" KeyPath="yes"/>
            </Component>
        </DirectoryRef>

        <!-- user PATH: placed in TARGETDIR (dir keypath) so it installs even if all
             file features are unchecked; native Environment table removes only our
             appended segment on uninstall (Permanent=no). -->
        <DirectoryRef Id="TARGETDIR">
            <Component Id="AddBoothToPath" Guid="{7726564C-F68A-48B6-9CE1-D283EACF716A}" KeyPath="yes">
                <Environment Id="AddBoothToPathEnv" Name="PATH" Value="[INSTALLDIR]" Action="set" Part="last" System="no" />
            </Component>
            <!-- 自注册位置变量：与 GUI 运行时自注册同语义（等值覆盖），卸载时按值清理 -->
            <Component Id="RegisterToolhubLocation" Guid="{D78CAD9C-C0A8-4AD9-8715-BFC6DB0E82E6}" KeyPath="yes">
                <Environment Id="ToolhubLocationEnv" Name="BOOTHVAULT_TOOLHUB" Value="[INSTALLDIR]" Action="set" System="no" />
            </Component>
        </DirectoryRef>

        <Feature Id="BoothCLI" Title="booth CLI" Level="1" Absent="allow">
            <ComponentRef Id="booth.exe"/>
        </Feature>
        <Feature Id="BoothMCP" Title="booth MCP server" Level="1" Absent="allow">
            <ComponentRef Id="booth_mcp.exe"/>
        </Feature>
        <Feature Id="BoothShell" Title="booth Shell" Level="1" Absent="allow">
            <ComponentRef Id="booth_shell.exe"/>
        </Feature>
        <Feature Id="AddToPath" Title="Add booth dir to user PATH" Level="1" Absent="allow">
            <ComponentRef Id="AddBoothToPath"/>
        </Feature>
    </Fragment>
</Wix>
`;

writeFileSync(join(scriptDir, "..", "src-tauri", "wix", "generated.wxs"), wxs, "utf8");
console.log(
  "[stage-cli] generated.wxs -> " + join(scriptDir, "..", "src-tauri", "wix", "generated.wxs")
);

const nsh = `; build-time generated: absolute paths of the 3 booth binaries, do not edit
!define BOOTH_CLI_SRC "${boothSrc}"
!define BOOTH_MCP_SRC "${boothMcpSrc}"
!define BOOTH_SHELL_SRC "${boothShellSrc}"
`;
const nshDir = join(projectOut, "nsis");
mkdirSync(nshDir, { recursive: true });
writeFileSync(join(nshDir, "generated.nsh"), nsh, "utf8");
console.log("[stage-cli] generated.nsh -> " + join(nshDir, "generated.nsh"));

const envarSrc = resolve(scriptDir, "..", "src-tauri", "nsis", "plugins", "EnVar.dll");
if (!existsSync(envarSrc)) {
  console.error(`[stage-cli] 缺少 EnVar.dll：${envarSrc}`);
  process.exit(1);
}
const pluginDir = join(nshDir, "plugins");
mkdirSync(pluginDir, { recursive: true });
cpSync(envarSrc, join(pluginDir, "EnVar.dll"));
console.log("[stage-cli] EnVar.dll -> " + join(pluginDir, "EnVar.dll"));
console.log("[stage-cli] 完成");
