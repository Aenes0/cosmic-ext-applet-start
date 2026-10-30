# Menu Iniciar redimensionável com layout responsivo (spec + tarefas)

Pedido do usuário (2026-10-22): "Permita também redimensionar o menu iniciar, sendo o tamanho máximo o
que está hoje e o mínimo uns 400px. Tudo dentro precisa reagir bem a isso."

Regras herdadas (ver specs anteriores de 2026-10-22): sem commit; a árvore de widgets não muda de
estrutura sem necessidade (trocar estrutura perde estado); ícones de app em `px(36)`; símbolos de
interface sem `ICON_SCALE`. Este documento é **spec + plano**; precisa de aprovação (G2) antes da Onda B.

Linhas citadas = `src/menu.rs` lido em 2026-10-22 (4120 linhas; outras tarefas o editam em paralelo, então
os números podem ter mudado — localizar pelo símbolo).

---

## 1. Decisões

### 1.1 Como redimensionar (a superfície continua em tela inteira)
- A layer surface **não muda**: continua `Anchor::TOP|BOTTOM|LEFT|RIGHT`, `size: (None, None)`. Só o
  retângulo do menu desenhado dentro dela muda. Nada de ida-e-volta com o compositor a cada passo.
- **Alças** = uma camada de `iced::widget::pin` posicionada sobre as bordas de `menu_rect`, cada alça um
  `widget::mouse_area(space)` com `.interaction(cursor)`, `.on_press(Message::ResizeStart(edge))` e
  `.on_double_press(Message::ResizeReset)`.
  - Espessura 8 px: **6 px para fora** (dentro da faixa `MARGIN = 16` da sombra, sobre o backdrop) e
    **2 px para dentro** (não roubar a barra de rolagem da `home`, que fica colada na borda direita).
  - Cantos: quadrados de 16 px (6 fora + 10 dentro). As alças laterais não cobrem os cantos.
  - Ordem no `stack` raiz: `[backdrop, placed, ALÇAS, anchored_popup?, ghost?, capa_de_resize?]` — a
    camada de alças é **sempre** a 3ª (estrutura fixa); os popups ancorados ficam acima dela.
  - Mesmo padrão que já funciona: o `mouse_area` do menu com `on_press(Nothing)` impede o backdrop de
    fechar o menu; a alça faz o mesmo para a parte de fora (verificar no teste manual).
- **Arrasto**: igual ao arrasto de apps. `ResizeStart(edge)` arma; uma subscription `event::listen_with`
  **só ativa enquanto `!resize.is_idle()`** emite `ResizeMove(window, position)` em `CursorMoved` e
  `ResizeEnd(window)` em `ButtonReleased(Left)`; ambos filtrados por `accepts_size(self.surface, id)`.
  Como `on_press` não dá o ponto, o **primeiro** `CursorMoved` fixa o ponto inicial (como `drag_fresh`) e
  o tamanho passa a ser calculado por **delta** a partir do tamanho no início — sem salto.
- **Cursor**: na alça, `mouse::Interaction`; durante o arrasto, uma **capa** (último layer, só enquanto
  redimensiona) `mouse_area` de tela inteira com o mesmo cursor — mantém o cursor fora da alça e impede
  hover/realce dos tiles. Mapeamento conferido em `iced/winit/src/conversion.rs` do libcosmic `d4d71fd`:
  `ResizingHorizontally`=EwResize, `ResizingVertically`=NsResize, `ResizingDiagonallyUp`=NeswResize (`/`),
  `ResizingDiagonallyDown`=NwseResize (`\`).
- **Âncora (decisão)**: o menu continua **centralizado na horizontal** e **preso à borda do painel** na
  vertical (é o que `menu_rect`/`placed` já fazem e o que mantém o menu sobre o botão Iniciar).
  - Menu embaixo (`on_top == false`): a borda **de baixo fica fixa**; mexem **topo, esquerda, direita** e
    os cantos **superior-esquerdo** (`\`, DiagonallyDown) e **superior-direito** (`/`, DiagonallyUp).
  - Menu em cima (`on_top == true`): a borda **de cima fica fixa**; mexem **base, esquerda, direita** e os
    cantos inferior-esquerdo (`/`, Up) e inferior-direito (`\`, Down).
  - Lados são **simétricos**: arrastar a borda direita `dx` muda a largura em `2·dx` (a outra borda anda
    junto, espelhada), então a borda pega acompanha o cursor exatamente e o centro não sai do lugar.
  - Justificativa: ancorar num lado deslocaria o menu do botão Iniciar e exigiria `menu_rect` com
    deslocamento horizontal persistido; a borda colada ao painel não tem alça (é o rodapé, encostado no
    painel), igual ao Windows 11.
  - `enum Edge { Left, Right, Far, FarLeft, FarRight }` ("Far" = a borda longe do painel).

### 1.2 Limites e persistência
- `MAX_WIDTH = 874` (o `WIDTH` de hoje), `MIN_WIDTH = 400`, `MAX_HEIGHT = 920` (o `HEIGHT`),
  `MIN_RESIZE_HEIGHT = 480` (abaixo disso o card de desinstalar e o popup de grupo não cabem com folga;
  480 = busca 80 + rodapé 64 + 336 de corpo, que comportam cabeçalho + 2 linhas de Fixados).
- Limites **efetivos** dependem da superfície (`room_width`, `room`), função pura:
  - `max_w = MAX_WIDTH.min(room_width − 2·MARGIN)` (se `room_width` desconhecido: `MAX_WIDTH`);
    `min_w = MIN_WIDTH.min(max_w)`.
  - `max_h = menu_height()` de hoje: `MAX_HEIGHT.min(room − 2·MARGIN).max(MIN_HEIGHT=300)`;
    `min_h = MIN_RESIZE_HEIGHT.min(max_h)` (tela baixa: o mínimo cede ao máximo).
- Estado novo: `menu_pref: Option<Size>` (o tamanho **pedido** pelo usuário) e `resize: resize::Resize`.
  `room`/`room_width` continuam sendo **a superfície**; o tamanho **do menu** é sempre
  `fn menu_size(&self) -> Size = resize::effective(menu_pref, limits(room_width, room))`. Nunca guardar o
  tamanho efetivo no lugar do pedido (trocar de monitor/tela baixa não pode encolher a preferência).
- **Persistência**: chave nova `menu_size: (u32, u32)` no `config()` (`Config::new(APP_ID, 1)`), gravada
  **só no `ResizeEnd`** e só se mudou. Leitura em `init`: valores fora de `[400..=874]×[480..=920]` são
  **clampados** (0/lixo → clamp; erro de leitura → `None`). Clamp de novo no uso (limites efetivos).
- **Reset**: duplo clique em qualquer alça → `menu_pref = None` e grava `(874, 920)` (o `ConfigSet` não
  tem "apagar"; o máximo clampado é exatamente o padrão).
- `Escape` durante o arrasto: volta ao tamanho do início e encerra (opcional, barato: o estado já guarda
  `from`).

### 1.3 Interações com o resto
- `ResizeStart` é **ignorado** se `!drag.is_idle()` ou `uninstall_running()`. Senão: `commit_name()`,
  `close_menu()` (popups ancorados teriam âncora velha), `cancel_drag()`.
- `DragArm` é **ignorado** enquanto `!resize.is_idle()`.
- `hide()` zera o `resize` (sem gravar). `Unfocused` não interfere.
- A cada mudança de tamanho: `open_offset` reajustado com a nova capacidade (ver §2, `adjust_open`).
- Prévia de janelas (tarefa paralela) e barra "Desafixar" (tarefa paralela): devem usar o método novo
  `self.menu_rect()` / `self.metrics()` — **nunca** `WIDTH`/`SIDE`. Quem entrar depois rebaseia.
- Override para prints: `COSMIC_START_MENU_SIZE=<w>x<h>` (mesmo espírito de `COSMIC_START_PREVIEW`) fixa
  `menu_pref` sem gravar — permite verificar 400/480/600/720 sem arrastar.

---

## 2. Layout responsivo — `src/menu/metrics.rs` (puro, sem iced)

`pub fn metrics(width: f32, height: f32) -> Metrics` (entrada defensivamente clampada a
`[MIN_WIDTH, MAX_WIDTH] × [300, MAX_HEIGHT]`), calculado **uma vez por `view_window`** e passado por
referência (`m: &Metrics`) às views. `fit(width, cell, gap) = max(1, floor((width + gap)/(cell + gap)))`
(o mesmo de `open_strip::fit`, que passa a ser reexportado daqui ou chamado de lá).

| Campo | Regra | 400 | 480 | 600 | 720 | 874 (hoje) |
|---|---|---|---|---|---|---|
| `side` (padding lateral; era `SIDE=46`) | `round(24 + (w−400)·22/474)` em `[24,46]` | 24 | 28 | 33 | 39 | **46** |
| `inner` | `w − 2·side` | 352 | 424 | 534 | 642 | **782** |
| `pinned_cols` (era 8) | `fit(inner, 72, 12).min(8)` (tile ≥ 72 > `PIN_BOX` 52) | 4 | 5 | 6 | 7 | **8** |
| `pinned_rows` | 2 (fixo) → `pinned_shown = cols·2` | 8 | 10 | 12 | 14 | **16** |
| `grid_cols` ("Todos" em grade, com nome; era `PINNED_COLUMNS`) | `fit(inner, 84, 12).min(8)` | 3 | 4 | 5 | 6 | **8** |
| `card_cols` (Recomendados; era 3) | `fit(inner, 200, 16).min(3)` | 1 | 2 | 2 | 3 | **3** |
| `card_rows` (era 2) | `ceil(6/cols).min(4)` | 4 | 3 | 3 | 2 | **2** |
| `card_title_chars` (era 25) | `floor((card_w − 75)/7)` em `[10,25]`, `card_w=(inner−16(c−1))/c` | 25 | 18 | 25 | 18 | **25** |
| `card_sub_chars` (era 28) | `floor((card_w − 75)/6.25)` em `[10,28]` | 28 | 20 | 28 | 20 | **28** |
| `category_cols` (era 4) | `fit(inner, 140, 16).min(4)` (≥ `FOLDER_BOX` 104 + folga) | 2 | 2 | 3 | 4 | **4** |
| `open_inner` (era `INNER=758`) | `inner − 24` (padding 12 da faixa) | 328 | 400 | 510 | 618 | **758** |
| → capacidade sem/com setas | `fit(open_inner,65,8)` / `fit(open_inner−72,65,8)` | 4/3 | 5/4 | 7/6 | 8/7 | **10/9** |
| `detail_panel` (busca) | `inner ≥ 520` | não | não | sim | sim | **sim** |
| `compact_headers` | `inner < 440` | sim | sim | não | não | **não** |
| `group_popup_width` (era 640) | `640.min(w − 48)` | 352 | 432 | 552 | 640 | **640** |
| `group_cols` (era 6) | `fit(group_popup_width − 8, 84, 4).min(6)` | 3 | 4 | 6 | 6 | **6** |
| `group_max_height` (era `HEIGHT·0.6`) | `0.6·h` | — | — | — | — | **552** (h=920) |
| `context_width` (era 260) | `260.min(w − 32)` | 260 | 260 | 260 | 260 | **260** |
| `power_width` (era 200) | `200` | 200 | 200 | 200 | 200 | **200** |
| `uninstall_width` (era 420) | `420.min(w − 32)` | 368 | 420 | 420 | 420 | **420** |
| `uninstall_max_height` | `h − 64` (corpo rola dentro) | — | — | — | — | 856 |
| `popup_side` (era `SIDE−16`) | `side − 16` | 8 | 12 | 17 | 23 | **30** |
| `footer_pad` (era `SIDE−12`) | `side − 12` | 12 | 16 | 21 | 27 | **34** |
| `user_chars` (nome no rodapé) | `floor((w − 2·footer_pad − 150)/8)` em `[8,40]` | 28 | 37 | 40 | 40 | 40 |
| `tools_width` (coluna do Win+X; era `WIDTH`) | `w.max(tools::WIDTH = 250+2+190 = 442)` | 442 | 480 | 600 | 720 | **874** |

Notas de comportamento por seção:
- **Fixados**: colunas e quantidade mostrada caem com a largura; "Mostrar tudo" aparece quando
  `pinned.len() > pinned_shown` (já é assim, só troca a constante). `visible_pinned`/`area_slot` recebem
  `shown: usize` (afeta `drag_specs` — o arrasto passa a medir os fixados visíveis da largura atual).
- **Recomendados**: 3 → 2 → 1 coluna; com menos colunas, mais linhas (6/6/4 itens); `ellipsize` dos
  cards usa `card_title_chars`/`card_sub_chars`. "Mostrar tudo" quando `picks.len() > cols·rows`. A página
  Recomendados usa `card_cols`.
- **Todos**: categorias 4→3→2; grade com `grid_cols`; lista não muda.
- **Abertos**: `open_strip::capacity(total, open_inner)`; `CELL/GAP/ARROW` fixos.
- **Busca**: com `detail_panel`, igual a hoje (6/5). Sem ele (`menu < ~600`): **só a lista, largura
  toda**; as ações do painel continuam acessíveis por **clique direito na linha** (popup ancorado
  `Target::App/Window/File` — o mesmo do resto do menu; linhas ganham id `scoped("hit", chave)`), Enter
  continua abrindo. Clique direito nas linhas vale em qualquer largura (inofensivo).
- **Cabeçalhos**: com `compact_headers`, a pílula de "Todos" mostra só o nome da vista
  ("Categoria ⌄") em vez de "Exibir como: Categoria ⌄"; "Mostrar tudo"/"Voltar" ficam.
- **Rodapé**: padding `footer_pad`; nome do usuário com `ellipsize(user, user_chars)`. Altura `FOOTER`
  fixa.
- **Popups**: contexto/power/desinstalar clampados à largura do menu (`popup_origin` assume
  `popup.width ≤ menu.width` — passa a ser verdade); grupo com largura e colunas derivadas, altura máxima
  `0.6·h`; card de desinstalar com o corpo em `scrollable` limitado a `uninstall_max_height`.
- **Win+X**: coluna `tools_width`, alinhada ao início — a borda esquerda das ferramentas acompanha a
  borda esquerda do menu (exceto abaixo de 442, onde fica 21 px para fora em vez de cortar o submenu).
- **Alturas**: `home`, Recomendados e lista da busca já rolam; `FOOTER`, cabeçalho 28 e `FOLDER_*` ficam.

**Teste dourado obrigatório**: `metrics(874.0, 920.0)` devolve exatamente os valores de hoje (coluna
874). A Onda B é refactor **sem mudança visual** no tamanho padrão.

---

## 3. Inventário de largura/altura fixa (49 pontos: 45 passam a vir de `Metrics`/limites, 4 ficam)

`src/menu.rs`:
1. `WIDTH` (L93) · 2. `HEIGHT` (L94) · 3. `MIN_HEIGHT` (L98, **fica** como piso de tela baixa) ·
4. `SIDE` (L99) · 5. `PINNED_COLUMNS` (L101) · 6–9. `PINNED_ROWS`, `CARD_COLUMNS`, `CARD_ROWS`,
`CATEGORY_COLUMNS` (L223–226)
10. `adjust_open` → `open_strip::capacity(total)` (L146–153) · 11. `Message::OpenPage` capacity (L802)
12. `menu_rect` usa `WIDTH` (L213–217) → `menu_rect(surface, on_top, size: Size)` + método `self.menu_rect()`
13. Win+X: coluna `Fixed(WIDTH)` (L997) · 14. barra de busca `padding([28, SIDE, 16, SIDE])` (L1025)
15. coluna do conteúdo `Fixed(WIDTH)` (L1030) · 16. container do menu `Fixed(WIDTH)`/`Fixed(height)` (L1038–1039)
17. `menu_height()` (L1475–1481) → `menu_size()`
18. `header` pílula sem modo compacto (L1709–1733) · 19. `app_tile` `ellipsize(13)` (L1747; ok com tile ≥ 84)
20. `pinned_grid` `PINNED_COLUMNS` (L1837–1847) · 21. `open_icon`/`open_arrow`/`open_strip` (L1875–1895)
22. `tile_grid(columns)` (L1924, repassa) · 23. `card` `ellipsize(25/28)` (L1954–1955)
24. `card_grid` `CARD_COLUMNS` (L1998–1999) · 25. `home` fixados mostrados (L2053, L2077)
26. `home` cards mostrados (L2093, L2108) · 27. `home` padding `SIDE` (L2131)
28. `recommended_page` `SIDE` (L2147, L2149) · 29. `categories` `CATEGORY_COLUMNS` (L2173–2174)
30. `all_grid` `PINNED_COLUMNS` (L2200) · 31. `results`: split 6/5, ícone 64, padding `SIDE` (L2337–2381)
32. `footer`: `SIDE − 12`, nome sem limite (L2407, L2439) · 33. `popup_card`: 260/420/`POPUP_WIDTH`/200 e
`Fixed(width)` (L2475, 2503, 2530, 2588, 2610) · 34. `popup_menu`: `SIDE − 16` (L2657–2658)
35. `anchored`/`anchored_popup`: `menu_rect(..., menu_height())` (L2677–2689)
36. `visible_pinned` (L3148) · 37. `area_slot` (L3168) · 38. `drag_specs` (L1163–1165, via 36/37)
39. testes `the_menu_sits_centered…` e `visible_pinned_and_the_area_slot` (L3902–3904, L3924)

`src/menu/group_ui.rs`: 40. `POPUP_WIDTH = 640` · 41. `POPUP_COLUMNS = 6` em `popup_body` ·
42. `max_height(HEIGHT * 0.6)` · 43. `FOLDER_BOX 104`/`FOLDER_CARD_HEIGHT 144` (**fica**; define o
mínimo de 140 da coluna de categoria) · 44. `PIN_BOX 52` (**fica**; tile mínimo 72) ·
45. `member_tile` `ellipsize(11)` (**fica**; coluna ≥ 84).
`src/menu/open_strip.rs`: 46. `INNER = 758` (`CELL/GAP/ARROW` ficam).
`src/menu/anchor.rs`: 47. `popup_origin` sem guarda para popup mais largo que o menu (resolvido pelo clamp
das larguras; adicionar teste).
`src/tools.rs`: 48. cards 250 + 190 (+2) = 442 > 400 → `pub const WIDTH`.
`src/menu/uninstall_ui.rs`: 49. card 420 (definido em `menu.rs`) e corpo sem rolagem (`MAX_LINES = 6`).

---

## 4. `src/menu/resize.rs` (puro)

```rust
pub enum Edge { Left, Right, Far, FarLeft, FarRight }
pub enum Resize { Idle, Armed { edge: Edge, from: Size }, Active { edge: Edge, from: Size, start: Point } }
pub struct Limits { pub min: Size, pub max: Size }
pub fn limits(room_width: f32, room: f32) -> Limits;                 // §1.2
pub fn effective(pref: Option<Size>, l: Limits) -> Size;             // None → max; clamp; arredonda
pub fn clamp_stored(w: u32, h: u32) -> Size;                         // leitura do config
pub fn resized(edge: Edge, from: Size, start: Point, now: Point, on_top: bool, l: Limits) -> Size;
//   Left: w = from.w − 2dx · Right: w = from.w + 2dx · Far: h = from.h − dy (embaixo) | + dy (em cima)
pub fn handles(menu: Rectangle, on_top: bool) -> [(Edge, Rectangle); 5];  // §1.1, sem sobreposição
pub fn interaction(edge: Edge, on_top: bool) -> mouse::Interaction;
impl Resize { arm, moved(point, on_top, l) -> (Self, Option<Size>), is_idle }
```
Testes: cada borda nas duas orientações; clamp nos dois limites; tela baixa (`room = 700` → max_h 668;
`room = 400` → min_h = max_h = 368); `room_width = 600` → max_w 568; `clamp_stored(0,0)` e
`(99999, 99999)`; primeiro movimento só fixa `start` (tamanho inalterado); alças cobrem as 3 bordas e 2
cantos longe do painel, nenhuma na borda do painel, sem interseção entre si; cursores dos cantos trocam
com `on_top`.

---

## 5. Ordem segura e ondas

O refactor de constantes mexe em ~40 lugares de `menu.rs` e colide com qualquer edição concorrente.
Princípio: **módulos novos em paralelo primeiro; `menu.rs` sempre em série, um operário por vez, com
`menu.rs` "travado" para as outras tarefas** (prévia de janelas, barra Desafixar, correções de grupos
precisam ter entrado ou esperado).

| Onda | Tarefa | Arquivos | Teste / verificação | Depende |
|---|---|---|---|---|
| **A** (paralelo) | A1 `metrics.rs` + tabela da §2 + teste dourado 874 + teste de monotonicidade (w 400..=874: colunas não decrescem; tile ≥ 72; card ≥ 200 se >1 coluna; categoria ≥ 140; popups ≤ menu) | `src/menu/metrics.rs` novo + linha `mod metrics;` | `cargo test metrics` | — |
| A | A2 `resize.rs` (§4) + testes | `src/menu/resize.rs` novo + `mod resize;` | `cargo test resize` | — |
| A | A3 `open_strip::capacity_in(total, inner)`; `capacity(total)` vira compat `= capacity_in(total, INNER)` até a Onda B; testes 328/400/510/618 | `src/menu/open_strip.rs` | `cargo test open_strip` | — |
| A | A4 `tools::WIDTH = 442.0` (constante pública, sem mudar `view`) + teste `anchor::popup_origin` com popup mais largo que o menu (documenta o comportamento) | `src/tools.rs`, `src/menu/anchor/tests.rs` | `cargo test` | — |
| **0** (opcional, precisa do OK do usuário) | dividir `menu.rs` (§6) | `menu.rs` → `menu/view_*.rs` | build + testes + print idêntico | parados os outros trabalhos em `menu.rs` |
| **B** (série, `menu.rs` exclusivo) | B1 geometria: `menu_pref` (ainda `None`), `menu_size()`, `menu_rect(Size)` + `self.menu_rect()`, `self.metrics()`, `visible_pinned/area_slot(shown)`, `adjust_open(prev_cap, cap, …)`, `OpenPage`. B2 views: todos os pontos 12–35 recebem `m: &Metrics`. B3 `group_ui::popup_body(…, cols, max_height)`, largura do grupo, coluna do Win+X. B4 **apagar** `WIDTH, HEIGHT, SIDE, PINNED_*, CARD_*, CATEGORY_COLUMNS, open_strip::INNER, POPUP_WIDTH, POPUP_COLUMNS` — o compilador acha o que sobrou | `menu.rs`, `group_ui.rs`, `open_strip.rs` | `cargo test` (todos) + `cargo clippy`; print padrão **idêntico** ao de antes | A1, A3, A4 |
| **C** (série) | Alças + arrasto + capa de cursor + `ResizeStart/Move/End/Reset` + subscription + guardas (§1.3) + `menu_size` no config + override `COSMIC_START_MENU_SIZE` | `menu.rs` | `cargo test`; prints com override 400/480/600/720 | B, A2 |
| **D** (D1→D2 em série; D3 em paralelo) | D1 busca estreita (sem painel + clique direito nas linhas). D2 cabeçalho compacto, `ellipsize` dos cards por `Metrics`, nome no rodapé. D3 corpo do card de desinstalar em `scrollable` com `max_height` (recebe o valor como parâmetro) | D1/D2 `menu.rs`; D3 `uninstall_ui.rs` (+1 linha de chamada em `menu.rs` feita em D2) | `cargo test`; prints 400/600/874 de busca, Todos, Recomendados, desinstalar | C |
| **E** | `revisor` (diff × spec) + teste manual do usuário | — | — | D |

Paralelizável sem `menu.rs`: só a Onda A (e D3). Se a Onda 0 for aprovada, D1 e D2 viram paralelas
(arquivos `view_search.rs` e `view_home.rs`/`view_chrome.rs` disjuntos).

Briefs dos operários (Sonnet, effort low; B e C podem pedir `medium` pelo tamanho) levam a **licença de
corrigir o plano**, "não commite", o trecho da §2/§4 relevante e as assinaturas reais.

---

## 6. Dividir `menu.rs` antes? (proposta, **não assumida**)

Proposta mínima: mover, sem mudar código, blocos `impl StartMenu { … }` para módulos filhos —
`menu/view_home.rs` (home, pinned_grid, tile_grid, card, card_grid, categories, all_grid, all_list,
app_row, open_strip/open_icon/open_arrow), `menu/view_search.rs` (results), `menu/view_chrome.rs`
(search_bar, header, back_header, footer, popup_card, popup_menu, anchored, anchored_popup). Em Rust, um
módulo filho enxerga os itens privados do pai: **zero mudança de visibilidade**, só recorte e colagem.

- **Prós**: `menu.rs` cai de ~4100 para ~2700 linhas (regra de 800 continua violada, mas menos);
  operários leem só o módulo da sua tarefa (menos contexto, menos erro); D1/D2 paralelos; conflitos
  futuros com outras tarefas ficam menores; a Onda B fica mais fácil de revisar por arquivo.
- **Contras**: diff grande que conflita com **todas** as edições em curso (prévia, Desafixar, grupos) — só
  num momento sem nenhum outro trabalho em `menu.rs`; ruído no `git blame`; mais um passo para verificar
  (build + testes + print idêntico); o usuário ainda não aprovou.
- **Recomendação**: fazer a Onda 0 **só se** o usuário aprovar **e** depois que prévia/Desafixar entrarem.
  Sem aprovação, o plano funciona igual — B, C e D apenas rodam em série.

---

## 7. Riscos

1. **Desempenho do arrasto**: cada `CursorMoved` refaz o layout do menu inteiro (a "Todos" em lista tem
   centenas de linhas). Mitigar: arredondar para px inteiro e ignorar movimento que não muda o tamanho;
   se travar, aplicar só a cada 16 ms (tick) com o último ponto. Só o usuário sente.
2. **Alça × barra de rolagem** na borda direita (2 px para dentro) e × tooltips nas bordas.
3. **Clique na parte de fora da alça fechar o menu** se o `stack` não capturar como esperado (o padrão
   atual sugere que captura; confirmar no teste manual).
4. **Cursor em layer surface**: o mapeamento existe no winit do libcosmic; o cosmic-comp precisa mostrar
   o formato (cursor-shape-v1). Se não aparecer, o resize funciona mas sem dica visual.
5. **Troca de estrutura** ao cruzar limiares (`detail_panel`, número de colunas): perde a rolagem da
   lista de busca ao cruzar 600 px — aceitável; não trocar estrutura fora desses limiares.
6. **Concorrência**: B e C precisam de `menu.rs` exclusivo; a prévia e a barra Desafixar precisam adotar
   `self.menu_rect()`/`Metrics` (quem entrar depois rebaseia).
7. `ellipsize` por número de caracteres é aproximação de largura em px — nomes largos (W, M) podem
   cortar com `clip` antes do "…".
8. Win+X abaixo de 442 px sai 21 px para a esquerda da borda do menu (escolha consciente para não cortar
   o submenu de energia).

## 8. O que só o usuário consegue testar
- Achar e pegar as alças (8 px, cantos 16 px), o formato do cursor e a sensação do arrasto (fluidez,
  simetria dos lados, borda do painel fixa), em baixo e com o painel em cima.
- O layout real em larguras intermediárias (os prints com `COSMIC_START_MENU_SIZE` ajudam, mas fontes e
  nomes reais decidem) e se 480 de altura mínima é confortável.
- Persistência ao fechar/reabrir, reset por duplo clique, tela baixa/outro monitor, Win+X na borda nova.
- Arrastar apps/grupos e abrir popups num menu estreito (popup de grupo com 3 colunas a 400 px).

## Ajustes da revisão Opus (2026-10-22) — OBRIGATÓRIOS (relatados por agente; conferir no código)
1. **HIGH — contrato único de `adjust_open`:** `adjust_open(prev_cap, cap, total, active, changed, offset)` (a prévia contava grupos; o resize, capacidade). Ordem única do stack raiz: `[backdrop, placed, alças, prévia (slot fixo), anchored_popup?, unpin_bar?, ghost?, capa?]` (a spec omitia a camada `unpin_bar`, que já existe, e a prévia; a prévia fica ACIMA das alças).
2. Clique na alça fora do menu NÃO fecha o menu (o stack para no `capture_event`; mouse_area com `on_press` captura). Durante arrasto de app, desligar `interaction`/`on_press` das alças mantendo a camada.
3. **MEDIUM — `ellipsize(13)`** é falso nos Fixados: o tile cai de 87 px para 75–81 px (400 a 720). Acrescentar `tile_chars` a `Metrics` (ou mínimo de 84 também nos Fixados).
4. **LOW:** `metrics()` limita w ≥ 400 mas `max_w` pode ficar < 400 (tela < 432 px) — já tratado com piso 1.0 em `limits()`. Antes do throttle, medir view+layout em Todos/Lista com `tracing`; se passar de 8 ms, limitar a 1× por frame (`window::frames`) em vez de `every(16ms)`.
5. Processo: sem base commitada, não há diff × spec nem rollback — recomendar commit nos portões (o usuário commita).
