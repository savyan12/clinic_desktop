import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { validateRuntimeFiles, WINDOWS_RUNTIME_FILES } from './prepare-runtime.mjs'

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'clinic-rust-runtime-test-'))
  t.after(() => rm(root, { recursive: true, force: true }))
  for (const relativePath of WINDOWS_RUNTIME_FILES) {
    const path = join(root, relativePath)
    await mkdir(dirname(path), { recursive: true })
    await writeFile(path, 'runtime fixture')
  }
  return root
}

test('the Rust release runtime requires only MySQL and the VC++ prerequisite', async (t) => {
  const root = await fixture(t)
  await validateRuntimeFiles(root)
  assert.equal(WINDOWS_RUNTIME_FILES.some((path) => path.includes('php') || path.includes('backend')), false)
})

test('a missing MySQL runtime file fails validation', async (t) => {
  const root = await fixture(t)
  await rm(join(root, 'mysql/bin/mysqld.exe'))
  await assert.rejects(validateRuntimeFiles(root), /mysqld\.exe/)
})

test('an empty prerequisite file fails validation', async (t) => {
  const root = await fixture(t)
  await writeFile(join(root, 'vcredist/VC_redist.x64.exe'), '')
  await assert.rejects(validateRuntimeFiles(root), /VC_redist\.x64\.exe/)
})
