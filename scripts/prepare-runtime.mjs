import { execFile } from 'node:child_process'
import { mkdir, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'

const projectRoot = resolve(import.meta.dirname, '..')
const runtimeRoot = resolve(projectRoot, 'src-tauri/resources/runtime')
const execFileAsync = promisify(execFile)

export const WINDOWS_RUNTIME_FILES = [
  'mysql/bin/mysqld.exe',
  'mysql/bin/mysql.exe',
  'mysql/bin/mysqldump.exe',
  'mysql/bin/mysqladmin.exe',
  'mysql/share/english/errmsg.sys',
  'vcredist/VC_redist.x64.exe',
]

export async function validateRuntimeFiles(root, files = WINDOWS_RUNTIME_FILES) {
  for (const relativePath of files) {
    const path = resolve(root, relativePath)
    const info = await stat(path).catch(() => null)
    if (!info?.isFile() || info.size === 0) {
      throw new Error(`Required runtime file is missing or empty: ${path}`)
    }
  }
}

async function main() {
  const requireWindowsRuntime = process.argv.includes('--require-windows-runtime') || process.env.CLINIC_RELEASE_BUILD === 'true'
  if (requireWindowsRuntime && process.platform !== 'win32') {
    throw new Error('The Windows NSIS installer must be prepared and built on Windows.')
  }
  await mkdir(runtimeRoot, { recursive: true })

  if (requireWindowsRuntime) {
    await validateRuntimeFiles(runtimeRoot)
    const { stdout, stderr } = await execFileAsync(process.execPath, [
      resolve(projectRoot, 'scripts/validate-windows-runtime.mjs'),
    ], { windowsHide: true })
    process.stdout.write(stdout)
    process.stderr.write(stderr)
  }
  console.log(`Rust desktop runtime prepared at ${runtimeRoot}`)
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main()
}
