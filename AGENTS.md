# Repository Guidelines

## Using Vite+, the Unified Toolchain for the Web

This project is using Vite+, a unified toolchain built on top of Vite, Rolldown, Vitest, tsdown, Oxlint, Oxfmt, and Vite Task. Vite+ wraps runtime management, package management, and frontend tooling in a single global CLI called `vp`. Vite+ is distinct from Vite, and it invokes Vite through `vp dev` and `vp build`. Run `vp help` to print a list of commands and `vp <command> --help` for information about a specific command.

Docs are local at `node_modules/vite-plus/docs` or online at https://viteplus.dev/guide/.

## Project Structure & Module Organization

This is a Nuxt 4, Vue 3, TypeScript Project that uses Vite Plus configuration. Application code lives in `app/`: route pages in `app/pages/`, layouts in `app/layouts/`, reusable components in `app/components/`, global CSS in `app/assets/css/`, and UI config in `app/app.config.ts`. Static public files belong in `public/`. Unit tests live in `test/unit/`, Nuxt runtime/component tests in `test/nuxt/`, and Playwright end-to-end specs in `tests/`. Keep generated artifacts such as `.nuxt/`, `.output/`, `coverage/`, `playwright-report/`, and `test-results/` out of source control.

## Build, Test, and Development Commands

Use `pnpm` as the package manager; the project declares `pnpm@11.9.0`.

- `vp i`: install dependencies.
- `vpr dev`: start the local Nuxt dev server.
- `vpr build`: create a production build.
- `vpr generate`: generate static output.
- `vpr preview`: preview the built app.
- `vpr fmt`: run `oxfmt` and its formatting check.
- `vpr lint`: run `oxlint`.
- `vpr typecheck`: run Nuxt type checking.
- `vpr test`: run all Vitest projects.
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

- Never run database-related scripts `db:*`

## Editing Guidance

- Make the smallest correct change.
- Do not polish unrelated code.
- Do not remove correct comments or documentation.
- Do not rename broad parts of the codebase unless required.
- Do not expand a change into a repo-wide refactor unless necessary.
- Prefer leaving correct existing code in place.
- When touching production-sensitive code, prioritize reliability over clever abstractions.

## Before Finishing

- Run `vpr fmt` if you changed formatting significantly.
- Run `vpr lint` or at least targeted `oxlint` on touched files.
- Run targeted tests when tests exist.
- For runtime-sensitive changes, prefer a narrow smoke check over broad refactors.
- If you changed build or runtime behavior, ensure `vpr build` still works.

## LLMS Links

- Nuxt: https://nuxt.com/llms.txt
- Nuxt UI: https://ui.nuxt.com/llms.txt
- Vue: https://vuejs.org/llms.txt

## Testing Guidelines

Use Vitest for unit and Nuxt-aware tests, with `@nuxt/test-utils` helpers such as `mountSuspended` when rendering Nuxt components. Name test files `*.test.ts` or `*.spec.ts`. Keep tests focused on observable behavior. Run targeted checks first, for example `pnpm test:unit -- test/unit/example.test.ts`, then broaden to `vpr test`, `vpr test:nuxt`, or `vpr test:e2e` when behavior crosses boundaries.

## Review Checklist

- [ ] Run `vp install` after pulling remote changes and before getting started.
- [ ] Run `vp check` and `vp test` to format, lint, type check and test changes.
- [ ] Check if there are `vite.config.ts` tasks or `package.json` scripts necessary for validation, run via `vp run <script>`.
- [ ] If setup, runtime, or package-manager behavior looks wrong, run `vp env doctor` and include its output when asking for help.
