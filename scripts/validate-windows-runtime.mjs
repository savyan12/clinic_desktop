import { execFile } from 'node:child_process'
import { open, readFile, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { promisify } from 'node:util'

const exec = promisify(execFile)
const projectRoot = resolve(import.meta.dirname, '..')
const runtimeRoot = resolve(projectRoot, 'src-tauri/resources/runtime')
const release = process.env.CLINIC_RELEASE_BUILD === 'true'
const lock = JSON.parse(await readFile(resolve(import.meta.dirname, 'windows-runtime.lock.json'), 'utf8'))

async function requireFile(path) {
  const info = await stat(path).catch(() => null)
  if (!info?.isFile() || info.size === 0) {
    throw new Error(`Required runtime file is missing or empty: ${path}`)
  }
}

async function requireX64(path) {
  const file = await open(path)
  try {
    const header = Buffer.alloc(64)
    await file.read(header, 0, header.length, 0)
    const pe = Buffer.alloc(6)
    await file.read(pe, 0, pe.length, header.readUInt32LE(60))
    if (header.readUInt16LE(0) !== 0x5a4d || pe.readUInt32LE(0) !== 0x4550 || pe.readUInt16LE(4) !== 0x8664) {
      throw new Error(`Expected a Windows x64 executable: ${path}`)
    }
  } finally {
    await file.close()
  }
}

async function run(binary, args) {
  try {
    return await exec(binary, args, { windowsHide: true, timeout: 120_000 })
  } catch (error) {
    throw new Error(`Runtime check failed (${binary}): ${error.stderr || error.stdout || error.message}`)
  }
}

if (process.platform !== 'win32' || process.arch !== 'x64') {
  throw new Error('Validate the Windows runtime on a Windows x64 build host.')
}

const executables = ['mysqld', 'mysql', 'mysqldump', 'mysqladmin'].map((name) => `mysql/bin/${name}.exe`)
for (const path of [...executables, 'vcredist/VC_redist.x64.exe', 'mysql/share/english/errmsg.sys']) {
  await requireFile(resolve(runtimeRoot, path))
}
for (const path of executables) await requireX64(resolve(runtimeRoot, path))

for (const name of ['mysqld', 'mysql', 'mysqldump', 'mysqladmin']) {
  const { stdout, stderr } = await run(resolve(runtimeRoot, `mysql/bin/${name}.exe`), ['--no-defaults', '--version'])
  const version = (stdout + stderr).match(/\b(?:Ver|Distrib) (8\.\d+\.\d+)\b/)
  if (!version || (release && version[1] !== lock.mysql.version)) {
    throw new Error(`Expected ${release ? lock.mysql.version : 'MySQL 8'} in ${name}.exe; found ${stdout}${stderr}`)
  }
  console.log(stdout.trim() || stderr.trim())
}

const config = JSON.parse(await readFile(resolve(projectRoot, 'src-tauri/tauri.conf.json'), 'utf8'))
const windowsConfig = JSON.parse(await readFile(resolve(projectRoot, 'src-tauri/tauri.windows.conf.json'), 'utf8'))
const resources = config.bundle.resources
const expectedResources = {
  'resources/runtime/mysql': 'runtime/mysql',
  'resources/runtime/vcredist': 'runtime/vcredist',
}
if (JSON.stringify(config.bundle.targets) !== '["nsis"]'
  || JSON.stringify(resources) !== JSON.stringify(expectedResources)
  || Object.keys(resources).some((path) => /(?:php|backend|composer|laravel)/i.test(path))
  || config.bundle.windows.webviewInstallMode.type !== 'offlineInstaller'
  || config.bundle.windows.nsis.installerHooks !== 'windows/installer-hooks.nsh') {
  throw new Error('Release must bundle only the Rust application, MySQL, VC++, and offline WebView2 through NSIS.')
}
if (typeof windowsConfig.productName !== 'string'
  || !windowsConfig.productName
  || !/^[\x20-\x7E]+$/.test(windowsConfig.productName)) {
  throw new Error('The Windows product name must be non-empty ASCII so the default MySQL installation path is supported.')
}
for (const path of [...config.bundle.icon, config.bundle.windows.nsis.installerHooks]) {
  await requireFile(resolve(projectRoot, 'src-tauri', path))
}

console.log(`Windows Rust runtime validation passed: MySQL ${lock.mysql.version} x64 and NSIS resources verified.`)
