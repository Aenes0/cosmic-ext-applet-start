# Desinstalar apps pelo menu (spec)

Item **"Desinstalar…"** no menu de contexto de um app (`Target::App`), seguro e uniforme para
Flatpak, pacman/AUR, Steam, Wine, Waydroid, AppImage e `.desktop` solto.

## Princípios
1. **Sem dono conhecido, sem botão.** Origem desconhecida nunca desinstala (não adivinhar).
2. **Sempre confirmar mostrando o que sai.** Nada é apagado direto; reversível quando possível
   (lixeira via `gio trash`).
3. **Dados do usuário são opt-in** (caixa desmarcada) onde o backend distingue app de dados.
4. **Privilégio só via `pkexec`** (polkit). O applet não guarda senha.
5. **Assíncrono**: detecção barata é pura; consultas ao pacman/waydroid rodam fora da thread de UI.
6. **Um só ponto de entrada**: `uninstall::detect` → `Origin`; `plan` → `Plan{label, preview, steps}`;
   `run(plan)`. A UI não conhece pacman/flatpak.

## Origens (ordem de detecção; a primeira que casa vence)
Entrada: caminho do `.desktop`, `Exec`, `appid`. Fatos verificados nesta máquina em 2026-10-22.

| # | Origem | Casa quando | Plano |
|---|---|---|---|
| 0 | **Protegido** | appid `io.github.Aenes0.*` ou `cosmic-*` | sem botão |
| 1 | Flatpak | caminho contém `/flatpak/exports/` | `flatpak uninstall -y --noninteractive [--user\|--system] [--delete-data] <appid>`; escopo pelo prefixo do caminho |
| 2 | Steam/Proton | `Exec` tem `steam://rungameid/<N>` | `steam steam://uninstall/<N>` (o Steam mostra o próprio diálogo) |
| 3 | Waydroid | `Exec` = `waydroid app launch <pkg>` | `waydroid app remove <pkg>`; depois move o `.desktop` residual para a lixeira. Exige sessão ativa: se parada, erro "Inicie o Waydroid" |
| 4 | Wine | `Exec` tem `WINEPREFIX=<dir>` e `wine` | `WINEPREFIX=<dir> wine uninstaller` (GUI "Adicionar/Remover" do Wine). **Nunca apaga o prefixo.** Prefixos Proton/launchers próprios (sem `WINEPREFIX=`) caem em "desconhecido" |
| 5 | AppImage | algum token do `Exec` termina em `.appimage` (sem caso) e é arquivo regular sob `$HOME` | `gio trash` do `.AppImage` + do `.desktop` |
| 6 | pacman/AUR | caminho sob `/usr/` e `pacman -Qqo <caminho>` devolve pacote | prévia `pacman -Rs -p --print-format '%n %v' <pkg>`; execução `pkexec pacman -Rs --noconfirm <pkg>`. Nunca `-Rdd`. Pacote exigido por outro/`base` → o próprio pacman recusa e o erro é mostrado. snap-pac já tira snapshot |
| 7 | `.desktop` solto | caminho sob `~/.local/share/applications`, nada acima casou | `gio trash` só do arquivo |
| 8 | Desconhecido | resto | sem botão |

## UI
- `popup_menu` (`Target::App`): item "Desinstalar…" só quando `detect` ≠ Desconhecido/Protegido.
- Novo `Target::Uninstall(id)`: card com título, linha "Origem: Flatpak/pacman/…", spinner enquanto
  `plan` roda, depois a lista "Será removido:" (prévia), caixa "Apagar também dados" (só Flatpak,
  desmarcada), botões **Cancelar** / **Desinstalar**. Durante a execução, spinner e botões inertes.
- Fim: sucesso → fecha, recarrega a lista de apps; erro → o card mostra o stderr (cortado) e fica
  aberto para o usuário ler.
- Textos em `texts.rs` (pt-BR e demais idiomas já suportados lá).

## Fora de escopo
Snap, Proton com prefixo próprio (Heroic/Lutris/Bottles), desinstalar do Android via ADB,
"ocultar do menu" para origem desconhecida (podemos fazer depois, é outro recurso).

## Testes
Funções puras com tabelas (detecção, parsing de `Exec`, montagem dos comandos) em
`src/uninstall.rs`; execução injetável por trait `Runner` para testar `plan`/`run` sem tocar o
sistema. Casos adversariais: `Exec` com aspas/escapes, `Exec` sem `wine` mas com `WINEPREFIX=`,
`.AppImage` fora do `$HOME`, appid protegido, pacote com dependentes, caminho com espaço.

## Ajustes da revisão Opus (2026-10-22) — obrigatórios
Relatados por agente; o implementador confere cada um no código/host antes de aplicar.
1. **Flatpak por `X-Flatpak`** (`DesktopEntry::flatpak()`), não pelo caminho: o `.desktop` em `~/.local/share/applications` pode sobrepor o de `exports/` (caso real: `com.anthropic.ClaudeDesktop`). Ref vem da chave; escopo pela existência de `~/.local/share/flatpak/app/<ref>` ou `/var/lib/flatpak/app/<ref>`. Entrada com `X-Flatpak-Part-Of` (PWAs do Brave) **nunca** desinstala o pai. Entrada solta cujo id existe em outro diretório de dados = override → sem botão.
2. **Waydroid:** `waydroid app remove` sai 0 mesmo em falha. Antes: checar `waydroid status` (sessão ativa). Depois: confirmar que o pacote sumiu de `waydroid app list` antes de mover o `.desktop`. Pacotes de sistema (`com.android.*`, `org.lineageos.*`) sem botão.
3. **Foco/fechamento do card:** com desinstalação em andamento, `Unfocused`, Escape e clique fora **não** fecham o card (`uninstall_running`); o resultado nunca se perde.
4. **Parsing seguro do `Exec`:** `shlex` (dependência direta), depois do unescape do `.desktop`; só `argv[0]` após desembrulhar `env`; AppImage: `canonicalize` + `starts_with($HOME canônico)` + `symlink_metadata().is_file()`; `gio trash -- <abs>`; validar Steam `^\d+$`, pacote Waydroid e ref Flatpak por regex; `pacman -Qqo -- <path>` e `/usr/bin/pacman` absoluto no `pkexec`; falha de parse → Desconhecido.
5. **Protegidos:** além de `io.github.Aenes0.*` e `cosmic-*`, também `com.system76.Cosmic*` e pacotes do grupo `cosmic`.
6. **Regra 7 (solto)** exclui `Exec` com wine/proton/flatpak/steam/`sh -c` (ex.: Autodesk Fusion) → Desconhecido.
7. **Wine:** `WINEPREFIX` precisa existir, ser do uid e ter `system.reg`.
8. **Steam:** `xdg-open steam://uninstall/N` (funciona com Steam Flatpak).
9. **pacman:** a prévia lista também os outros apps (`.desktop`) do mesmo pacote que saem junto.
10. **Recarga:** no sucesso, forçar releitura (`known=None`) e remover o id de `pinned`/`launched`; `apps_signature` não vê subpastas profundas.
11. **Origem calculada em `load_apps`** (já roda em `spawn_blocking`) e guardada no `AppEntry`; a view não faz I/O.
12. **Testes extras:** override esconde Flatpak; `X-Flatpak-Part-Of`; Waydroid sai 0 em falha; `..`/symlink; token começando com `-`; Fusion via proton; `com.system76.Cosmic*`.
