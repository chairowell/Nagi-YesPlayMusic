import { afterEach, describe, expect, setDefaultTimeout, test } from 'bun:test';
import { execFile } from 'node:child_process';
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

import {
  buildSidecarCompliance,
  defaultComplianceOutput,
  defaultCompleteSourceOutput,
  EXPECTED_UNM_CRATES,
  type CargoMetadata,
  type CargoPackageMetadata,
} from '../scripts/build-sidecar-compliance.mjs';

const execFileAsync = promisify(execFile);
const projectRoot = path.resolve(import.meta.dir, '..');
const temporaryDirectories: string[] = [];

setDefaultTimeout(30_000);

afterEach(async () => {
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map(directory => rm(directory, { recursive: true, force: true }))
  );
});

async function createPackage(
  registryRoot: string,
  name: string,
  version: string,
  license: string,
  repository: string
): Promise<CargoPackageMetadata> {
  const packageRoot = path.join(registryRoot, `${name}-${version}`);
  await mkdir(path.join(packageRoot, 'src'), { recursive: true });
  await writeFile(
    path.join(packageRoot, 'Cargo.toml'),
    `[package]\nname = "${name}"\nversion = "${version}"\nedition = "2021"\nlicense = "${license}"\nrepository = "${repository}"\n`,
    'utf8'
  );
  await writeFile(path.join(packageRoot, 'src', 'lib.rs'), '', 'utf8');
  await mkdir(path.join(packageRoot, 'src', 'target'), { recursive: true });
  await writeFile(
    path.join(packageRoot, 'src', 'target', 'generated.rs'),
    '// source directory named target must not be mistaken for build output\n',
    'utf8'
  );
  await writeFile(
    path.join(packageRoot, 'NOTICE'),
    `${name} fixture notice\n`,
    'utf8'
  );
  return {
    id: `${name} ${version}`,
    name,
    version,
    license,
    authors: [`${name} contributors`],
    repository,
    manifest_path: path.join(packageRoot, 'Cargo.toml'),
    source: 'registry+https://github.com/rust-lang/crates.io-index',
  };
}

async function createFixture(): Promise<{
  root: string;
  metadata: CargoMetadata;
}> {
  const root = await mkdtemp(path.join(os.tmpdir(), 'ypm-compliance-test-'));
  temporaryDirectories.push(root);
  const sidecarRoot = path.join(root, 'src-tauri', 'sidecar');
  const coreRoot = path.join(root, 'src-tauri', 'core');
  const registryRoot = path.join(root, 'registry');
  await mkdir(path.join(sidecarRoot, 'src'), { recursive: true });
  await mkdir(path.join(coreRoot, 'src'), { recursive: true });
  await mkdir(path.join(root, 'src-tauri', 'src'), { recursive: true });
  await mkdir(path.join(root, 'src'), { recursive: true });
  await mkdir(path.join(root, 'legal'), { recursive: true });
  await cp(
    path.join(projectRoot, 'legal', 'GPL-3.0.txt'),
    path.join(root, 'legal', 'GPL-3.0.txt')
  );
  await cp(
    path.join(projectRoot, 'legal', 'LGPL-3.0.txt'),
    path.join(root, 'legal', 'LGPL-3.0.txt')
  );
  await writeFile(path.join(root, 'LICENSE'), 'fixture MIT license\n', 'utf8');
  await writeFile(
    path.join(root, 'src-tauri', 'src', 'main.rs'),
    'fn main() {}\n',
    'utf8'
  );
  await writeFile(path.join(root, 'src-tauri', 'build.rs'), 'fn main() {}\n');
  await writeFile(
    path.join(root, 'src-tauri', 'Cargo.toml'),
    '[workspace]\nmembers = ["core", "sidecar"]\nresolver = "2"\n\n[profile.release]\ncodegen-units = 1\nlto = true\nopt-level = "s"\npanic = "abort"\nstrip = true\n',
    'utf8'
  );
  await writeFile(
    path.join(root, 'src', 'sidecar-route-manifest.json'),
    '[]\n',
    'utf8'
  );

  const dependencyLines = [
    ...EXPECTED_UNM_CRATES.map(name => `${name} = "=0.4.0"`),
    'random-string = "=1.1.0"',
  ];
  await writeFile(
    path.join(sidecarRoot, 'Cargo.toml'),
    `[package]\nname = "yesplaymusic-sidecar"\nversion = "0.7.0"\nedition = "2021"\nrust-version = "1.89"\nlicense = "GPL-3.0-only"\n\n[dependencies]\nyesplaymusic-core = { path = "../core" }\n${dependencyLines.join(
      '\n'
    )}\n`,
    'utf8'
  );
  await writeFile(path.join(sidecarRoot, 'src', 'main.rs'), 'fn main() {}\n');
  await writeFile(
    path.join(coreRoot, 'Cargo.toml'),
    '[package]\nname = "yesplaymusic-core"\nversion = "0.7.0"\nedition = "2021"\nrust-version = "1.89"\nlicense = "GPL-3.0-only"\n',
    'utf8'
  );
  await writeFile(
    path.join(coreRoot, 'src', 'lib.rs'),
    'pub const FIXTURE: &str = "core";\n',
    'utf8'
  );

  const unmRepository = 'https://github.com/UnblockNeteaseMusic/server-rust';
  const dependencies = await Promise.all([
    ...EXPECTED_UNM_CRATES.map(name =>
      createPackage(
        registryRoot,
        name,
        '0.4.0',
        'LGPL-3.0-or-later',
        unmRepository
      )
    ),
    createPackage(
      registryRoot,
      'random-string',
      '1.1.0',
      'GPL-3.0-only',
      'https://github.com/DmitrijVC/random-string'
    ),
  ]);
  const rootPackage: CargoPackageMetadata = {
    id: 'yesplaymusic-sidecar 0.7.0',
    name: 'yesplaymusic-sidecar',
    version: '0.7.0',
    license: 'GPL-3.0-only',
    authors: [],
    repository: null,
    manifest_path: path.join(sidecarRoot, 'Cargo.toml'),
    rust_version: '1.89',
  };
  const corePackage: CargoPackageMetadata = {
    id: 'yesplaymusic-core 0.7.0',
    name: 'yesplaymusic-core',
    version: '0.7.0',
    license: 'GPL-3.0-only',
    authors: [],
    repository: null,
    manifest_path: path.join(coreRoot, 'Cargo.toml'),
    rust_version: '1.89',
  };

  const lockPackages = dependencies
    .map(
      ({ name, version }) =>
        `[[package]]\nname = "${name}"\nversion = "${version}"\nchecksum = "${'a'.repeat(
          64
        )}"\n`
    )
    .join('\n');
  await writeFile(
    path.join(root, 'src-tauri', 'Cargo.lock'),
    `# fixture lock\nversion = 4\n\n[[package]]\nname = "yesplaymusic-core"\nversion = "0.7.0"\n\n${lockPackages}`,
    'utf8'
  );

  return {
    root,
    metadata: {
      packages: [rootPackage, corePackage, ...dependencies],
      resolve: {
        nodes: [
          {
            id: rootPackage.id,
            deps: [
              { pkg: corePackage.id },
              ...dependencies.map(({ id }) => ({ pkg: id })),
            ],
          },
          { id: corePackage.id, deps: [] },
          ...dependencies.map(({ id }) => ({ id, deps: [] })),
        ],
      },
    },
  };
}

async function createOptionalDependencyFixture(): Promise<{
  root: string;
  metadata: CargoMetadata;
  sourceMetadata: CargoMetadata;
}> {
  const fixture = await createFixture();
  const coreManifest = path.join(
    fixture.root,
    'src-tauri',
    'core',
    'Cargo.toml'
  );
  await writeFile(
    coreManifest,
    `${await readFile(coreManifest, 'utf8')}\n[features]\ndefault = []\nfixture-cache = ["dep:optional-fixture"]\n\n[dependencies]\noptional-fixture = { version = "=1.0.0", optional = true }\n`,
    'utf8'
  );

  const optionalPackage = await createPackage(
    path.join(fixture.root, 'registry'),
    'optional-fixture',
    '1.0.0',
    'MIT',
    'https://example.invalid/optional-fixture'
  );
  await writeFile(
    path.join(fixture.root, 'src-tauri', 'Cargo.lock'),
    `${await readFile(
      path.join(fixture.root, 'src-tauri', 'Cargo.lock'),
      'utf8'
    )}\n[[package]]\nname = "optional-fixture"\nversion = "1.0.0"\nchecksum = "${'b'.repeat(64)}"\n`,
    'utf8'
  );

  const corePackage = fixture.metadata.packages.find(
    candidate => candidate.name === 'yesplaymusic-core'
  );
  if (!corePackage) throw new Error('missing core fixture package');
  const sourceMetadata: CargoMetadata = {
    packages: [...fixture.metadata.packages, optionalPackage],
    resolve: {
      nodes: [
        ...fixture.metadata.resolve.nodes.map(node =>
          node.id === corePackage.id
            ? {
                ...node,
                deps: [
                  ...node.deps,
                  { pkg: optionalPackage.id, dep_kinds: [{ kind: null }] },
                ],
              }
            : node
        ),
        { id: optionalPackage.id, deps: [] },
      ],
    },
  };
  return { ...fixture, sourceMetadata };
}

describe('Rust Sidecar copyleft distribution bundle', () => {
  test('complete source rebuild resolves disabled optional dependencies offline', async () => {
    const fixture = await createOptionalDependencyFixture();
    const { stdout } = await execFileAsync('rustc', ['-vV']);
    const targetTriple = stdout.match(/^host: (.+)$/m)?.[1];
    if (!targetTriple) throw new Error('rustc did not report its host triple');

    const outputDirectory = path.join(fixture.root, 'optional-output');
    const completeSourceDirectory = path.join(
      fixture.root,
      'optional-complete-source'
    );
    const result = await buildSidecarCompliance({
      projectRoot: fixture.root,
      outputDirectory,
      completeSourceDirectory,
      metadata: fixture.metadata,
      sourceMetadata: fixture.sourceMetadata,
      binaryProvenance: {
        targetTriple,
        fileName: `yesplaymusic-sidecar-${targetTriple}`,
        sha256: 'c'.repeat(64),
        rustMarker: 'YPM_RUST_SIDECAR_V1',
        machOUuid: null,
      },
    });

    expect(result.dependencyCount).toBe(13);
    const manifest = JSON.parse(
      await readFile(path.join(outputDirectory, 'SOURCE-MANIFEST.json'), 'utf8')
    ) as {
      completeSource: { offlineRebuildVerified: boolean };
      dependencySourcePackages: Array<{ name: string }>;
    };
    expect(manifest.completeSource.offlineRebuildVerified).toBe(true);
    expect(
      manifest.dependencySourcePackages.map(({ name }) => name)
    ).not.toContain('optional-fixture');
  });

  test('refuses linked output ancestors without deleting the external target', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'ypm-compliance-link-'));
    temporaryDirectories.push(root);
    const allowedDirectory = path.join(root, 'allowed');
    const externalDirectory = path.join(root, 'external');
    const protectedOutput = path.join(externalDirectory, 'sidecar-compliance');
    const sentinel = path.join(protectedOutput, 'keep.txt');
    await mkdir(allowedDirectory);
    await mkdir(protectedOutput, { recursive: true });
    await writeFile(sentinel, 'must survive\n', 'utf8');
    const linkedParent = path.join(allowedDirectory, 'redirect');
    await symlink(
      externalDirectory,
      linkedParent,
      process.platform === 'win32' ? 'junction' : 'dir'
    );

    await expect(
      buildSidecarCompliance({
        projectRoot: root,
        outputDirectory: path.join(linkedParent, 'sidecar-compliance'),
        metadata: { packages: [], resolve: { nodes: [] } },
        skipOfflineRebuild: true,
      })
    ).rejects.toThrow('symbolic-link or reparse-point ancestor');
    expect(await readFile(sentinel, 'utf8')).toBe('must survive\n');
  });

  test('builder produces verifiable GPL/LGPL source and relinking materials', async () => {
    const fixture = await createFixture();
    const outputDirectory = path.join(fixture.root, 'generated-output');
    const completeSourceDirectory = path.join(
      fixture.root,
      'generated-complete-source'
    );
    const result = await buildSidecarCompliance({
      projectRoot: fixture.root,
      outputDirectory,
      completeSourceDirectory,
      metadata: fixture.metadata,
      binaryProvenance: {
        targetTriple: 'aarch64-apple-darwin',
        fileName: 'yesplaymusic-sidecar-aarch64-apple-darwin',
        sha256: 'a'.repeat(64),
        rustMarker: 'YPM_RUST_SIDECAR_V1',
        machOUuid: '00112233-4455-6677-8899-AABBCCDDEEFF',
      },
      skipOfflineRebuild: true,
    });

    expect(result.copyleftSourceCount).toBe(13);
    expect(result.dependencyCount).toBe(13);
    expect(result.completeSourceDirectory).toBe(completeSourceDirectory);

    const manifest = JSON.parse(
      await readFile(path.join(outputDirectory, 'SOURCE-MANIFEST.json'), 'utf8')
    ) as {
      sidecar: {
        name: string;
        version: string;
        license: string;
        rustVersion: string;
      };
      copyleftSourcePackages: Array<{ name: string; license: string }>;
      dependencySourcePackages: Array<{ name: string; license: string }>;
      completeSource: {
        dependencySourceCount: number;
        offlineRebuildVerified: boolean;
      };
      dependencyNoticeCount: number;
    };
    expect(manifest.sidecar).toEqual({
      name: 'yesplaymusic-sidecar',
      version: '0.7.0',
      license: 'GPL-3.0-only',
      rustVersion: '1.89',
    });
    expect(manifest.copyleftSourcePackages.map(({ name }) => name)).toEqual([
      'random-string',
      ...EXPECTED_UNM_CRATES,
    ]);
    expect(manifest.dependencySourcePackages).toHaveLength(13);
    expect(
      manifest.dependencySourcePackages.map(({ name }) => name)
    ).not.toContain('yesplaymusic-core');
    expect(manifest.completeSource).toEqual(
      expect.objectContaining({
        dependencySourceCount: 13,
        offlineRebuildVerified: false,
      })
    );
    expect(manifest.dependencyNoticeCount).toBe(13);

    for (const packageName of EXPECTED_UNM_CRATES) {
      const bundledSource = path.join(
        completeSourceDirectory,
        'source',
        'vendor',
        `${packageName}-0.4.0`,
        'src',
        'lib.rs'
      );
      expect(await readFile(bundledSource, 'utf8')).toBe('');
      expect(
        await readFile(
          path.join(
            completeSourceDirectory,
            'source',
            'vendor',
            `${packageName}-0.4.0`,
            'src',
            'target',
            'generated.rs'
          ),
          'utf8'
        )
      ).toContain('must not be mistaken for build output');
    }
    expect(
      await readFile(path.join(outputDirectory, 'GPL-3.0.txt'), 'utf8')
    ).toContain('GNU GENERAL PUBLIC LICENSE');
    expect(
      await readFile(path.join(outputDirectory, 'LGPL-3.0.txt'), 'utf8')
    ).toContain('GNU LESSER GENERAL PUBLIC LICENSE');
    const thirdPartyNotices = await readFile(
      path.join(outputDirectory, 'THIRD-PARTY-NOTICES.md'),
      'utf8'
    );
    expect(thirdPartyNotices).toContain('random-string');
    expect(thirdPartyNotices).not.toContain('yesplaymusic-core');
    const standaloneManifest = await readFile(
      path.join(
        completeSourceDirectory,
        'source',
        'application',
        'src-tauri',
        'Cargo.toml'
      ),
      'utf8'
    );
    expect(standaloneManifest).toContain('members = ["core", "sidecar"]');
    expect(standaloneManifest).toContain('lto = true');
    const bundledCoreManifest = await readFile(
      path.join(
        completeSourceDirectory,
        'source',
        'application',
        'src-tauri',
        'core',
        'Cargo.toml'
      ),
      'utf8'
    );
    expect(bundledCoreManifest).toContain('name = "yesplaymusic-core"');
    expect(
      await readFile(
        path.join(
          completeSourceDirectory,
          'source',
          'application',
          'src-tauri',
          'core',
          'Cargo.toml.release'
        ),
        'utf8'
      )
    ).toBe(bundledCoreManifest);
    expect(
      await readFile(
        path.join(
          completeSourceDirectory,
          'source',
          'application',
          'src-tauri',
          'core',
          'src',
          'lib.rs'
        ),
        'utf8'
      )
    ).toContain('FIXTURE');
    await expect(
      readFile(
        path.join(
          completeSourceDirectory,
          'source',
          'vendor',
          'yesplaymusic-core-0.7.0',
          'Cargo.toml'
        ),
        'utf8'
      )
    ).rejects.toThrow();
    expect(
      await readFile(
        path.join(completeSourceDirectory, '.cargo', 'config.toml'),
        'utf8'
      )
    ).toContain('offline = true');
    expect(
      await readFile(path.join(completeSourceDirectory, 'rebuild.sh'), 'utf8')
    ).toContain('--offline --locked');
    const powershellVerifier = await readFile(
      path.join(completeSourceDirectory, 'verify-sources.ps1'),
      'utf8'
    );
    expect(powershellVerifier).toContain(
      '[System.Security.Cryptography.SHA256]::Create()'
    );
    expect(powershellVerifier).toContain(
      'OpenRead((Join-Path $PSScriptRoot $filePath))'
    );
    expect(powershellVerifier).not.toContain('Get-FileHash');
    await expect(
      readFile(path.join(outputDirectory, 'source', 'vendor'))
    ).rejects.toThrow();

    const verifierInvocationDirectory = path.join(
      fixture.root,
      'verifier-invocation'
    );
    await mkdir(verifierInvocationDirectory);

    // Invoke from outside the bundle so every checker must resolve its own files.
    await (process.platform === 'win32'
      ? execFileAsync(
          'powershell.exe',
          [
            '-NoProfile',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            path.join(completeSourceDirectory, 'verify-sources.ps1'),
          ],
          { cwd: verifierInvocationDirectory }
        )
      : execFileAsync(
          path.join(completeSourceDirectory, 'verify-sources.sh'),
          [],
          { cwd: verifierInvocationDirectory }
        ));
  });

  test('Tauri maps the generated bundle into every platform package', async () => {
    const tauriConfig = JSON.parse(
      await readFile(
        path.join(projectRoot, 'src-tauri', 'tauri.conf.json'),
        'utf8'
      )
    ) as {
      bundle: { resources: Record<string, string> };
    };
    const configuredSource = Object.entries(tauriConfig.bundle.resources).find(
      ([, destination]) => destination === 'sidecar-compliance/'
    )?.[0];
    expect(configuredSource).toBe('generated/sidecar-compliance/');
    expect(path.resolve(projectRoot, 'src-tauri', configuredSource ?? '')).toBe(
      defaultComplianceOutput
    );
    expect(configuredSource).not.toContain(
      path.basename(defaultCompleteSourceOutput)
    );
  });
});
