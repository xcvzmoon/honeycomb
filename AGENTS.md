# Repository Guidelines

## Using Vite+, the Unified Toolchain for the Web

This project is using Vite+, a unified toolchain built on top of Vite, Rolldown, Vitest, tsdown, Oxlint, Oxfmt, and Vite Task. Vite+ wraps runtime management, package management, and frontend tooling in a single global CLI called `vp`. Vite+ is distinct from Vite, and it invokes Vite through `vp dev` and `vp build`. Run `vp help` to print a list of commands and `vp <command> --help` for information about a specific command.

Docs are local at `node_modules/vite-plus/docs` or online at https://viteplus.dev/guide/.

## Project Structure & Module Organization

This is a Tauri v2 desktop app with a Nuxt 4, Vue 3, TypeScript frontend that uses Vite+ configuration. Application code lives in `app/`: route pages in `app/pages/`, layouts in `app/layouts/`, reusable components in `app/components/`, global CSS in `app/assets/css/`, and UI config in `app/app.config.ts`. Static public files belong in `public/`. Tauri/Rust code lives in `src-tauri/`, with app logic in `src-tauri/src/lib.rs`, the thin binary entry in `src-tauri/src/main.rs`, configuration in `src-tauri/tauri.conf.json`, and permissions in `src-tauri/capabilities/`. Release helper scripts live in `scripts/` and must use Node APIs, not Bun APIs. Unit tests live in `test/unit/`, Nuxt runtime/component tests in `test/nuxt/`, and Playwright end-to-end specs in `tests/`. Keep generated artifacts such as `.nuxt/`, `.output/`, `dist/`, `coverage/`, `playwright-report/`, `test-results/`, and `src-tauri/target/` out of source control.

## Build, Test, and Development Commands

Use `pnpm` as the package manager; the project declares `pnpm@11.9.0`. Do not use Bun commands or Bun-specific globals.

- `vp i`: install dependencies.
- `vpr dev`: start the Tauri dev app, which runs Nuxt through Tauri's `beforeDevCommand`.
- `vpr build`: run both `nuxt:build` and `tauri:build`.
- `vpr nuxt:dev`: start only the Nuxt dev server.
- `vpr nuxt:build`: build the Nuxt app.
- `vpr nuxt:generate`: generate static frontend output into `dist/` for Tauri.
- `vpr nuxt:preview`: preview the generated/built Nuxt app.
- `vpr nuxt:fmt`: run Vite+/Oxfmt formatting and formatting check.
- `vpr nuxt:lint`: run Vite+/Oxlint.
- `vpr nuxt:typecheck`: run Nuxt type checking.
- `vpr tauri:dev`: start Tauri development mode.
- `vpr tauri:build`: create a production Tauri build.
- `vpr tauri:fmt`: format Rust code with `cargo fmt`.
- `vpr tauri:lint`: lint Rust code with `cargo clippy -- -D warnings`.
- `vpr test`: run all Vitest projects.
- `vpr test:unit`: run unit tests.
- `vpr test:nuxt`: run Nuxt-aware tests.
- `vpr test:e2e`: run Playwright browser tests.

## Coding Style

- Prefer type-only imports with `import type`.
- Prefer `function` keyword over `const` in declaring functions.

## File Naming Conventions

- Prefer `camelCase` that starts with `use` for composables, ex: useFunctionName.ts
- Prefer `kebab-case` for pages, ex: my-page.vue
- Prefer `PascalCase` for components, ex: TheComponent.vue
- Prefer `kebab-case` as default for ts, json, html, and css

## CLI Instructions

- Never run database-related scripts `db:*`.
- Use `vp`/`vpr` and `pnpm` for package scripts; do not use `bun`, `bunx`, or Bun runtime APIs.
- Keep Nuxt and Tauri validation separate when possible: use `nuxt:*` scripts for frontend-only changes and `tauri:*`/Cargo commands for Tauri or Rust changes.
- For Tauri CI smoke checks, prefer `pnpm exec tauri build --no-bundle` before full platform packaging.

## Editing Guidance

- Make the smallest correct change.
- Do not polish unrelated code.
- Do not remove correct comments or documentation.
- Do not rename broad parts of the codebase unless required.
- Do not expand a change into a repo-wide refactor unless necessary.
- Prefer leaving correct existing code in place.
- When touching production-sensitive code, prioritize reliability over clever abstractions.

## Release Workflow

Release scripts are Node-based and align release commits/tags with the generated package version.

- Use `vpr release:patch`, `vpr release:minor`, or `vpr release:major` to create a release bump.
- The release flow runs `changelogen --bump`, syncs the version into Tauri files, creates a commit like `chore(release): v0.0.1`, creates tag `v0.0.1`, and pushes commits/tags.
- `vpr sync:tauri-version` syncs `package.json` version into `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, then refreshes Cargo metadata/lockfile.
- Do not hand-edit release commits/tags unless recovering from a failed release.

## Before Finishing

- Run `vpr nuxt:fmt` if you changed frontend formatting significantly.
- Run `vpr nuxt:lint` or at least targeted `oxlint` on touched frontend files.
- Run `vpr tauri:fmt` and `vpr tauri:lint` when touching Rust/Tauri files.
- Run targeted tests when tests exist.
- For runtime-sensitive changes, prefer a narrow smoke check over broad refactors.
- If you changed Nuxt build behavior, ensure `vpr nuxt:generate` still works.
- If you changed Tauri build/runtime behavior, ensure `pnpm exec tauri build --no-bundle` or `vpr tauri:build` still works.

## LLMS Links

- Nuxt: https://nuxt.com/llms.txt
- Nuxt UI: https://ui.nuxt.com/llms.txt
- Vue: https://vuejs.org/llms.txt
- Tauri V2: https://v2.tauri.app/llms.txt

## Testing Guidelines

Use Vitest for unit and Nuxt-aware tests, with `@nuxt/test-utils` helpers such as `mountSuspended` when rendering Nuxt components. Name test files `*.test.ts` or `*.spec.ts`. Keep tests focused on observable behavior. Run targeted checks first, for example `pnpm test:unit -- test/unit/example.test.ts`, then broaden to `vpr test`, `vpr test:nuxt`, or `vpr test:e2e` when behavior crosses boundaries.

## Review Checklist

- [ ] Run `vp i` after pulling remote changes and before getting started.
- [ ] For frontend changes, run relevant `nuxt:*` scripts such as `vpr nuxt:fmt`, `vpr nuxt:lint`, `vpr nuxt:typecheck`, and `vpr test`.
- [ ] For Tauri/Rust changes, run relevant `tauri:*` scripts such as `vpr tauri:fmt`, `vpr tauri:lint`, `cd src-tauri && cargo test`, and a Tauri build smoke check when needed.
- [ ] Check if there are `vite.config.ts` tasks or `package.json` scripts necessary for validation, run via `vpr <script>`.
- [ ] If setup, runtime, or package-manager behavior looks wrong, run `vp env doctor` and include its output when asking for help.
