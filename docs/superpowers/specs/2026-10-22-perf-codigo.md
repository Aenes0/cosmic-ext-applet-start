# Desempenho do código do menu — pontos quentes por quadro e por evento (2026-10-22)

> Escopo: só leitura e medição, numa **cópia** (`target/exp-src` = com o ajuste nº 2 aplicado;
> `target/exp-src0` = linha de base), `CARGO_TARGET_DIR=target/exp-cod`, perfil `perf`
> (= release sem LTO, `panic=unwind`). Nada foi alterado no repositório além deste arquivo.
> Ganhos rotulados **[medido]** vêm da sonda; **[estimado]**/**[hipótese]** não foram medidos na tela.

## 1. O que custa uma mensagem (fato confirmado no código do iced)

`iced/winit/src/lib.rs:1519-1534` (libcosmic d4d71fd): em cada iteração do laço com **qualquer**
mensagem (inclusive `Message::Nothing`), o iced chama `update` para todas e depois
`build_user_interfaces` = `view_window` + `diff` + `layout` do menu **inteiro**, e pede um redraw.
Mensagens da mesma iteração compartilham uma reconstrução; iterações diferentes, não.
Logo o custo por evento de ponteiro = custo de uma reconstrução completa.

## 2. Medições na cópia (host, 73 apps visíveis de ~180 .desktop, 12 fixados, 40 recentes, 10 janelas sintéticas em 9 grupos, superfície 3440×1400, menu 874×920)

Sonda: `src/menu/perf_probe.rs` + alocador contador em `main.rs` (só na cópia). Mediana de 30–200
execuções; `rebuild` = `view_window` + `UserInterface::build` (diff+layout) com renderizador
tiny-skia headless e cache quente; alocações são determinísticas.

| Cenário (por mensagem)                         | Base: µs / alocs | Com ajuste nº 2: µs / alocs |
|------------------------------------------------|------------------|-----------------------------|
| rebuild Início, Categoria                      | 314–342 / 6 728  | 210–213 / 3 175             |
| rebuild Início, Lista                          | 471–475 / 9 257  | 358–364 / 5 704             |
| rebuild Início, Grade                          | 553–560 / 10 330 | 434–436 / 6 777             |
| rebuild Recomendados                           | 147–152 / 2 391  | 145 / 2 391                 |
| rebuild busca "co"                             | 86 / 1 314       | 86 / 1 296                  |
| ResizeMove 1 px + rebuild (Cat/Lista/Grade)    | 336 / 540 / 628  | 237 / 430 / 512–534         |
| ResizeMove **mesmo ponto** + rebuild (Cat/Lista)| 313 / 471       | 212 / 365                   |
| DragMove + rebuild (Lista)                     | 464              | 353                         |
| PreviewMouse + rebuild (Cat)                   | 314              | 210                         |
| `Windows(lista idêntica)` + rebuild (Cat)      | 353 / 8 138      | 221 / 3 282                 |
| 219 apps (×3, ids sem casamento exato): Cat/Grade/Lista | 1 158 / 1 810 / 1 690 | 305 / 966 / 839 |
| draw (só gravar primitivas): Início            | 17–20            | —                           |

Peças isoladas (base): `pinned_items()` 29 µs / 1 069 alocs; `app_for_window` × 10 janelas
30 µs / 1 313 alocs (chamado 2× por grupo aberto por view); `hits()` 31–53 µs / 146–214 alocs;
`picks()` 9,7 µs; `groups.visible`+`group_apps` 5,9 µs; `scoped` × 73 = 10,7 µs; `ellipsize` × 73 =
8,9 µs; `flat_class` × 73 = 5,3 µs / 292 alocs; `palette(&theme::active())` 0,0 µs / 0 alocs.
Ícone: `named_icon` **falha a frio = 17,0 ms** (192 alocs, varre os temas); falha em cache 0,3 µs;
acerto 0,2 µs. O cache de falhas do `freedesktop-icons` (`cache.rs`/`lib.rs:270`) **expira em 5 s**.
Carga (fora da thread da UI): `load_apps` 488 ms a frio (1ª vez, resolve ícones) / 5,3 ms quente,
18 447 alocs; `load_recents` 2,9 ms / 9 233 alocs por show; `apps_signature` 85 µs por show;
`config()` 11 µs. Memória: `AppEntry` 360 B; 73 apps ≈ 120 KiB vivos; haystack 7,8 KB.

Orçamento: quadro de 144 Hz = 6,9 ms. Uma reconstrução (0,2–0,6 ms) cabe com folga; o risco vem de
(a) uma busca de ícone a frio de 17 ms dentro da view, (b) muitas reconstruções por quadro quando o
ponteiro gera eventos a 500–1000 Hz, (c) desenho na GPU (não medido aqui).

## 3. Pontos quentes ranqueados

| # | Ponto quente | Arquivo:linha | Frequência | Ganho | Risco | Teste que protege |
|---|---|---|---|---|---|---|
| 1 | `window_icon` → `named_icon(app_id…)` para janela sem app conhecido: falha refeita a cada 5 s **dentro da view** | `menu.rs:507-512`; usado em `tiles.rs:147`, `search.rs:152,199`, `preview_card.rs:85` | 1ª reconstrução após cada janela de 5 s, por app_id desconhecido (jogos Steam, apps sem .desktop) | **17 ms por app_id [medido o custo; frequência = hipótese]** → 0 | baixo | novo: ícone de janela desconhecida resolvido 1× e reaproveitado (resolvedor injetado conta chamadas); cache esvaziado em `Message::Apps` |
| 2 | `find_app`/`app_for_window`/`loosely`: `to_lowercase()` de cada app + `format!(".{x}")` por comparação | `load.rs:97-107`, `menu.rs:493-505`; chamados por `pinned_items` (`menu.rs:515`), `group_icon` 2×/grupo, `window_key` | **toda reconstrução** | **[medido] −33 % Cat, −24 % Lista, −22 % Grade; −53 % alocs; −74 % com 219 apps** | baixo (362 testes verdes na cópia) | `dotted_suffix` ≡ `ends_with(format!)` (feito na cópia, casos ASCII/UTF-8/vazios); tabela de `find_app`/`app_for_window` (id exato, maiúsculas, wm_class, sufixo `.x`, falso positivo `acalculator`) |
| 3 | Eventos de ponteiro = 1 reconstrução cada: `ResizeMove` (`application.rs:360`), `DragMove`, `PreviewMouse` (`preview_ui.rs:206`, escuta **todo** CursorMoved enquanto a prévia não está Idle) | `resize_glue.rs:83`, `application.rs:445`, `preview_ui.rs:74` | por evento de mouse (até a taxa do mouse) | [estimado] a 1000 Hz: 21–53 % de um núcleo só em rebuild; ponto repetido custa igual ao novo (313 vs 336 µs) | médio | puro: "último ponto pendente" aplicado 1× por tick e **descarregado no release**; ponto idêntico descartado |
| 4 | `toplevels::report` envia a lista inteira a cada `update_toplevel` (título, estado), mesmo igual | `toplevels.rs:112-134` → `application.rs:252` | por mudança de título com o menu aberto | [medido] 353 µs + redraw por evento inútil → 0 | baixo | puro `should_send(prev, &next)`; `#[derive(PartialEq)]` em `Window` |
| 5 | `hits()` dentro da view: `fold(título)`/`fold(nome)` por janela/arquivo e `to_lowercase()` no comparador do sort | `search.rs:23-67` (chamado por `results` e de novo em `Move`/`Submit`) | por reconstrução durante a busca | [medido] 17–53 µs → ~10 µs [estimado] | baixo (manter a MESMA chave de desempate) | ordem de `hits()` igual antes/depois para consultas fixas |
| 6 | `file_icon` (lookup por nome) por cartão de arquivo, a cada view | `tiles.rs:336`, `search.rs:156,211` | por reconstrução (Início: ≤ cartões visíveis; página Recomendados: até 30) | [medido] 20 µs/40 arquivos quente; falha a frio 17 ms a cada 5 s se o 1º mime faltar (0 de 40 aqui) | baixo | resolver em `load_recents` (thread de bloqueio) e guardar `icon::Handle` em `RecentFile` |
| 7 | Seção "Todos" (Lista/Grade) reconstruída e relayoutada inteira; por tile: 4 `Box` de `flat_class`, `format!` de `scoped`, 3 clones de `id`, clone de `icon::Handle` (PathBuf) | `pages.rs:200-248`, `tiles.rs:49-79` | toda reconstrução | view Lista/Grade ≈ 216/253 µs após nº 2; layout ≈ 145/180 µs | médio (`lazy` exige elemento `'static`) | só depois de medir nº 3 na tela; testes de snapshot de ids (`scoped`) e de mensagens |
| 8 | `categories()`/`pinned_items`: `app_keys()` recriado por grupo; `icons_of` clona todos os ícones do grupo (só ≤ 9 aparecem) | `pages.rs:166-195`, `menu.rs:544-551`, `tiles.rs:4` | toda reconstrução | [medido] ~6 µs | baixo | testes de `groups` existentes |
| 9 | I/O no show | `load.rs:4,124-143`, `recent_files.rs:42-117` | por show, fora da UI | pequeno (2,9 ms + 85 µs fora da UI) | baixo | — (não priorizar) |

Refutados pela leitura/medição (não são gargalo): `palette(&theme::active())` por view (0 µs, 0 aloc);
`open_groups`/`regroup` (já calculados só em `Message::Windows`/`Apps`, não por view);
`config()` (só por gesto: fixar, soltar, trocar vista, fim do resize, lançar); `thumbs` (limpos no
hide e no fechamento da prévia, 400 px máx.); `launched` (truncado em 12); `AppEntry` redundante
(`folded_name` + `haystack` ≈ 10 KB no total — economia irrelevante).

## 4. Ajustes propostos (ordem de execução)

1. **Ícone de janela sem app (nº 1).** Campo `window_icons: HashMap<String, icon::Handle>` (chave:
   `app_id` em minúsculas), preenchido em `set_windows` só para janelas sem app; `window_icon` lê
   dele; esvaziado em `Message::Apps`. Mantém o mesmo ícone/fallback visível. Opcional: resolver
   fora da thread da UI. Mesmo tratamento para `file_icon` (nº 6), resolvido em `load_recents`.
2. **Busca de app sem alocar (nº 2)** — medido na cópia: `AppEntry.id_lower` e `wm_class_lower`
   calculados em `load_apps`; `find_app`/`app_for_window` comparam com eles; o `.ends_with(format!)`
   vira `dotted_suffix(hay, wanted)` (sufixo + byte `.` antes). Resultado idêntico (minúsculas Unicode
   iguais, só calculadas uma vez). Passo seguinte opcional: guardar o índice do app por janela em
   `set_windows`/`regroup_same_windows`, eliminando `app_for_window` da view.
3. **Deduplicar `Windows` (nº 4)** em `toplevels::report`: guardar a última lista enviada e não enviar
   se igual (`Window: PartialEq`). Sem efeito visível.
4. **Ponteiro (nº 3), em duas etapas.** (a) seguro: descartar `CursorMoved` com a mesma posição
   do anterior (estático atômico no `listen_with`, que só aceita `fn`). (b) se a medição na tela
   mostrar > 1 evento por quadro: durante resize/arraste, o `listen_with` só grava o último ponto
   (atômico) e devolve `None`; um `time::every(~7 ms)` ativo **apenas durante o gesto** aplica o
   ponto pendente se houver; `ResizeEnd`/`DragRelease` aplicam o pendente antes de concluir. Não
   recalcular métricas se o tamanho arredondado não mudou (hoje já há guarda de igualdade em
   `resize_moved`, mas a reconstrução ocorre de qualquer modo).
5. **`hits()` (nº 5):** `fold` do título guardado em `Window` ao receber `Windows`, do nome em
   `RecentFile`; desempate com `sort_by_cached_key` (mesma chave `to_lowercase`).
6. Nº 7–8 só se, após 1–5, a tela ainda mostrar perda de quadros em Lista/Grade.

## 5. Testes novos propostos

- `dotted_suffix_matches_the_formatted_form` (já escrito e verde na cópia).
- `find_app_matches_as_before`: tabela com id exato, caixa diferente, `wm_class`, sufixo `.calculator`,
  rejeição de `gnome` em `org.gnome.Calculator` e de `acalculator`; idem `app_for_window`.
- `unknown_window_icon_is_resolved_once`: resolvedor injetado com contador; 2 views → 1 chamada;
  `Message::Apps` esvazia o cache.
- `report_skips_an_identical_list` (função pura `should_send`).
- `pending_pointer_is_flushed_on_release` e `same_point_is_dropped` (lógica pura do ponteiro).
- `hits_order_unchanged` para consultas fixas antes/depois do cache de `fold`.

## 6. O que depende de ver na tela (não medido aqui)

- Taxa real de `CursorMoved` por segundo durante resize/arraste/hover da prévia (contador de
  mensagens no binário instalado) — decide se o ajuste 4(b) vale a pena.
- Custo de desenho na GPU (wgpu: preparo de texto/atlas) ao redimensionar em Lista/Grade; a sonda só
  mediu a gravação de primitivas (17–20 µs) e o layout na CPU.
- Se há, na sessão real, janelas cujo `app_id` não casa com nenhum .desktop (gatilho do nº 1) e
  com que frequência os títulos mudam com o menu aberto (gatilho do nº 4).

Reproduzir: `flatpak-spawn --host sh -c 'cd target/exp-src && CARGO_TARGET_DIR=…/target/exp-cod
cargo test --offline --profile perf perf_ -- --nocapture --test-threads=1'` (idem `exp-src0` = base).
