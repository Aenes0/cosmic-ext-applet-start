# Desinstalar apps pelo menu (plano)

Spec: `docs/superpowers/specs/2026-10-22-desinstalar-apps.md`. Branch: `main` (escolha do usuário),
sem commit. Rota solo/TDD serial (arquivos acoplados em `menu.rs`).

Verificação padrão: `cargo test` e `cargo build --release` no repo (via `flatpak-spawn --host`).

| # | Tarefa | Arquivos | Verificação |
|---|---|---|---|
| 1 | `AppEntry` guarda `path`, `flatpak_ref`, `flatpak_part_of` e `origin` (calculado em `load_apps`, ver ajustes 1/11 da spec) | `src/menu.rs` | compila; testes existentes verdes |
| 2 | RED→GREEN: `Origin` + `detect()` puro (tabela da spec, ordem 0–8), parsing de `Exec` (aspas, `env`, `sh -c`) | `src/uninstall.rs` (novo), `src/main.rs` (`mod`) | testes de tabela, incluindo adversariais |
| 3 | RED→GREEN: `Plan` e `plan()` por origem com `Runner` injetável (pacman `-Qo`/`-Rs -p`, checagem de sessão Waydroid) | `src/uninstall.rs` | testes com `Runner` falso |
| 4 | RED→GREEN: `run(plan)` — comandos exatos por backend, `pkexec` só no pacman, pós-passo Waydroid, erro com stderr cortado | `src/uninstall.rs` | testes de montagem dos comandos (sem executar) |
| 5 | Mensagens/estado: `Target::Uninstall`, `Message::{AskUninstall, UninstallPlan, ToggleData, ConfirmUninstall, UninstallDone}`, campos de estado | `src/menu.rs` | compila |
| 6 | UI: item no `popup_menu`, card de confirmação/progresso/erro, textos | `src/menu.rs`, `src/texts.rs` | `cargo build --release`; teste de render não-regressivo se houver padrão (ex.: ícones) |
| 7 | Recarga da lista após sucesso (reutilizar `apps_signature`/`Apps`) | `src/menu.rs` | manual |
| 8 | Teste real no desktop com itens **descartáveis**: `.desktop` solto criado para o teste, AppImage fictício em `~/`, Flatpak de teste; Wine/Waydroid/pacman só até a tela de confirmação (sem executar) | — | screenshot + saída dos comandos |
| 9 | Revisão (`code-reviewer`, `security-reviewer`: injeção de argumentos via `Exec`/appid, path traversal no trash) e correções | — | findings tratados |

Riscos conhecidos:
- `waydroid app remove` pode já remover o `.desktop`; o pós-passo é idempotente.
- Se `Exec` do Wine for incomum, cai em Desconhecido (seguro).
- Nenhum comando de remoção real é executado nos testes automáticos.

Ajustes da revisão Opus: ver seção "Ajustes da revisão Opus" da spec — valem como requisitos das tarefas 1–9 (tarefa 7 também limpa `pinned`/`launched`; tarefa 6 inclui `uninstall_running`).
Execução: operários `sonnet` low, sequenciais por arquivo (`uninstall.rs` → `menu.rs`); sem commit.
