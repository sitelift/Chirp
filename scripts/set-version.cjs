const fs = require('node:fs')
const version = process.argv[2]
if (!/^2\.\d+\.\d+(?:-[\w.-]+)?$/.test(version)) throw new Error('Expected a v2 semver without v prefix')
for (const file of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) {
  const data = JSON.parse(fs.readFileSync(file, 'utf8'))
  data.version = version
  if (data.packages?.['']) data.packages[''].version = version
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n')
}
for (const file of ['src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']) {
  const data = fs.readFileSync(file, 'utf8').replace(/(name = "chirp"\r?\n(?:default-run = "chirp"\r?\n)?version = ")[^"]+/, '$1' + version)
  fs.writeFileSync(file, data)
}
