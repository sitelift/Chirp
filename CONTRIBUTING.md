# Contributing

Read [AGENTS.md](AGENTS.md) and [docs/v2/DIRECTION.md](docs/v2/DIRECTION.md).
Chirp v2 targets Windows; paste latency and transcript quality come first.

Branch from `v2`, keep changes focused, and explain the user-visible problem.
Follow the README build instructions. Run `cargo test`, `cargo build`,
`npm.cmd run build`, and `npm.cmd run lint` for relevant changes. Describe manual
Windows validation, including CPU/GPU selection for native inference changes.
Measure performance with `bench/`; do not infer latency from code alone.

Never commit API keys, private signing keys, recordings, model files, or personal
transcripts. Use synthetic fixtures or explicitly approved examples. Claude
input is untrusted data; errors must preserve deterministic dictation.

Contributions use MIT. Do not modify or redistribute reserved brand artwork;
read [BRAND.md](BRAND.md). Discuss major architecture or scope changes first.
