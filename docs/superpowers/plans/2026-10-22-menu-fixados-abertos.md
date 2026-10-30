# Fixados e Abertos em ícones, arrastar, menu junto ao item (plano)

Spec: `docs/superpowers/specs/2026-10-22-menu-fixados-abertos.md` (revisada pelo Opus: REPROVADA a 1ª
versão das §4/§5; reescrita incorporando os achados). Branch `main`, sem commit.
Verificação padrão: `cargo test` e `cargo build --release` (via `flatpak-spawn --host`).
Agentes: `operario` sonnet low (contrato §2.4). Tarefas em `menu.rs` são sequenciais; `drag.rs` e
`anchor.rs` são arquivos novos.

| # | Tarefa | Arquivos | Quem | Verificação |
|---|---|---|---|---|
| 1 | `ICON_SCALE`/`px` + aplicar aos ícones de app/janela; declarar `mod drag;` e `mod anchor;` (arquivos vazios/stub) | `menu.rs`, `menu/drag.rs`, `menu/anchor.rs` | op-uninstall-ui | teste de `px`; build |
| 2 | `pinned_tile` (só ícone + tooltip c/ atraso) só nos Fixados | `menu.rs` | op-uninstall-ui | build; print |
| 3 | Faixa cinza de Abertos (`open_icon`) acima dos Fixados | `menu.rs` | op-uninstall-ui | build; print |
| 5 | `drag.rs` puro: estados, `target_at`, `apply`, TDD | `menu/drag.rs` | operário novo (paralelo a 1–3) | testes do módulo |
| 4a | `anchor.rs`: `Operation` de bounds correta + função pura de posição do popup (TDD da posição) | `menu/anchor.rs` | operário novo (paralelo) | testes de posição; compila |
| 4b | Ligar: ids por escopo, `MenuFor/MenuAt`, tamanho da superfície, popup posicionado, `self.menu` só no `MenuAt` | `menu.rs` | op-uninstall-ui (após 1–3 e 4a) | build; print |
| 6a | Arrasto: mensagens, estado, subscription ativa só fora de `Idle`, cancelar em Esc/Hide/Unfocused | `menu.rs` | op-uninstall-ui (após 4b e 5) | build |
| 6b | Origens (`on_press_down_with_rectange`, sem `on_press` quando ativo) e consulta dos alvos | `menu.rs` | op-uninstall-ui | build |
| 6c | Fantasma, realce do alvo, origem esmaecida, `save_pinned` | `menu.rs` | op-uninstall-ui | build; teste manual do usuário |
| 7 | Revisão (code-reviewer opus) e correções | — | — | findings tratados |
| 8 | Prints (Fixados, Abertos, popup ao lado do item) | — | eu | imagens |

Riscos: edição concorrente de `menu.rs` (mitigado: 1–3, 4b, 6 sequenciais, módulos novos em paralelo);
arrasto sem harness de input (teste manual do usuário); `Operation` copiada de código com defeito
(testar a travessia com árvore real via screenshot do popup).
