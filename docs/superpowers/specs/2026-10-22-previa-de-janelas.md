# Abertos agrupados por app + prévia de janelas ao passar o mouse (spec)

> Data: 2026-10-22 · Repo: `~/Projetos/start-menu` (crate `cosmic-ext-applet-start`, Rust 2024, libcosmic/iced)
> Referência: `~/Projetos/taskbar` (crate `cosmic-ext-applet-taskbar`) — **somente leitura, não muda nada lá**.
> Pedido do usuário: _"Nos abertos, quero o mesmo efeito da barra de tarefas: ao passar o mouse mostra o
> mesmo preview das janelas que a barra de tarefas tem e acumula as janelas que pertencem ao mesmo app em
> uma coisa só. Se temos hoje um applet somente para preview, ele vai passar a fazer parte do applet do
> menu iniciar."_

## 0. Fatos verificados (base das decisões)

| Fato | Onde |
|---|---|
| **Não existe applet só de prévia.** A prévia vive dentro da barra: `taskbar/src/app.rs` (estado `hover`, `hover_gen`, `preview_hovered`, `HoverSettled`, `open_preview`/`close_preview`, `toplevel_button`, `capture_windows`, `RefreshPreview`), `wayland_handler.rs` (captura) e `wayland_subscription.rs` (`WaylandImage`, `WaylandRequest::Screencopy`). A barra **continua** com a prévia. | taskbar |
| Barra: abrir após **400 ms** (`HOVER_OPEN`), fechar **300 ms** após sair (`HOVER_CLOSE`), atualizar miniaturas a cada **1000 ms** (`PREVIEW_REFRESH`) só enquanto a prévia está aberta. | `app.rs:569-572, 2714` |
| Cartão da barra: **224×164**, cabeçalho 24 px (ícone 16 + título 1 linha com reticências + botão fechar 16 só no hover), imagem `ContentFit::Contain` abaixo; janela minimizada sem imagem ⇒ ícone 48; clique = ativa; botão do meio = fecha; selecionado se a janela está focada; até 7 por linha. | `app.rs:565-641, 1049-1068, 2519-2556` |
| Barra: clique num app com 1 janela ⇒ ativa/minimiza; com várias ⇒ abre a prévia na hora (`GroupClick`). | `app.rs:340-352, 1284-1292` |
| Captura: `cctk::screencopy` = **ext-image-copy-capture-v1** + **ext-foreign-toplevel-image-capture-source-v1** (fonte `CaptureSource::Toplevel(ExtForeignToplevelHandleV1)`), buffer **wl_shm Abgr8888** num `memfd`, sessão nova por foto, uma thread por foto, miniatura ≤ **400 px** no lado maior (média 2×2 a cada k px + `imageops::thumbnail`), entregue como `Handle::from_rgba`. | `wayland_handler.rs:274-555` |
| No cosmic-comp (checkout `742b800`), `ImageCopyCaptureState` e `ToplevelCaptureSourceState` usam o filtro `client_not_sandboxed` — **não exigem o socket privilegiado**. O menu roda fora do painel (`spawn_clean` remove `X_PRIVILEGED_WAYLAND_SOCKET`), não-sandbox ⇒ os globais aparecem. Instalado: cosmic-comp/panel/applets **1.10.0**. shm oferece `Xbgr8888` e `Abgr8888`. | `cosmic-comp/src/state.rs:686-695` |
| Pins diferentes: taskbar `cosmic-protocols` **c0cff4d** + libcosmic 5a8bd94; start-menu **32283d7** + libcosmic d4d71fd. Entre 32283d7 e c0cff4d o `client-toolkit/` só muda `Cargo.toml` ⇒ **API de screencopy idêntica**. O start-menu usa `cosmic::cctk` (reexport), não depende de `cctk` direto. | `Cargo.lock` dos dois; `git diff --stat` |
| `Handle::from_rgba(w, h, impl Into<Bytes>)` existe no libcosmic d4d71fd. | `iced/core/src/image.rs:148` |
| Os dois perfis release têm `panic = "abort"`: **um `unwrap()` que falhe numa thread de captura derruba o menu inteiro.** A barra tem `unwrap()` em `create_session`, `flush`, `wait_while(...).unwrap()`. | `Cargo.toml`, `wayland_handler.rs:390-447` |
| Defeito latente da barra: `ScreencopyHandler::stopped` é no-op e `wait_while` não tem timeout ⇒ se a sessão para antes de `init_done`/`ready` (janela fechou), a thread fica bloqueada para sempre e o handle fica em `capturing` ⇒ aquela janela nunca mais é fotografada. | `wayland_handler.rs:393-447, 607` |
| Menu: superfície layer-shell **em tela inteira** (`Anchor` nos 4 lados, `KeyboardInteractivity::Exclusive`); popups são **camadas `pin` dentro da mesma superfície** (`anchored_popup` + `anchor::popup_origin`), não xdg_popup. A faixa Abertos fica dentro do `scrollable` `HOME`. | `menu.rs:1078-1083, 1429-1439, 2128-2136, 2682-2694` |
| Lista de janelas: thread `run_windows` com `ToplevelInfoState` + `ToplevelManagerState`; `Window { id (identifier), title, app_id, active }`; só reporta com o menu visível (`WINDOWS_SHOWN`). | `menu.rs:295-300, 3470-3602` |

### 0.1 O bug da prévia presa (memória `taskbar-previa-presa`) — o que NÃO repetir

Sintoma: as prévias param de abrir até um clique no painel. Hipótese registrada: `self.popup` guarda
uma prévia/menu que o compositor já fechou e `menu_open()`/`preview_of()` barram o `HoverSettled`; e
cada `on_enter`/`on_exit` chega **2×**. Lendo `app.rs:1342-1361`, há dois mecanismos de "trava":

1. `HoverSettled` faz `return Task::none()` quando `preview_hovered && preview_of().is_some()` — se o
   `PreviewHover(false)` se perde (o widget não vê o ponteiro sair ao sair rápido), `preview_hovered`
   fica `true` e **nenhum timer futuro fecha** a prévia.
2. `HoverSettled` faz `return Task::none()` quando `menu_open()` — se `self.popup` ficou com um popup que
   já não existe, **nenhum timer futuro abre** a prévia.

Em ambos, o timer "decide não decidir" e o estado só sai com um evento externo (clique). **Regras desta
spec (invariantes testados):**

- **I1** — Todo timer que dispara com a geração corrente **resolve** para `Shown` ou `Idle`; nunca fica no
  mesmo estado esperando outro evento.
- **I2** — "Ponteiro dentro" é **derivado da posição** (último `CursorMoved` na superfície contra os
  retângulos do ícone e da prévia), não de um booleano trancado por pares enter/exit.
- **I3** — Bloqueios (menu de contexto aberto, arrasto, busca digitada, diálogo de desinstalar, menu
  escondido) **levam a `Idle`**, nunca a "esperar".
- **I4** — A prévia é uma camada da superfície do menu (não um xdg_popup): não há id de popup que o
  compositor possa destruir sem avisar. `hide()` sempre zera a prévia.
- **I5** — Eventos duplicados (enter/exit 2×) são idempotentes.

## 1. Decisões

### D1 — Copiar o mínimo para o start-menu, **não** extrair crate agora

Recomendação: **cópia adaptada** (~300 linhas de captura + ~150 de cartão) em módulos novos do
start-menu; a barra **não é tocada**.

| Critério | Cópia no start-menu | Crate compartilhado por path |
|---|---|---|
| Risco para a barra | zero (nenhum arquivo da barra muda) | refatora `wayland_handler.rs`/`app.rs` da barra, que está sob investigação do bug com logs `DIAG` |
| Pins | cada um mantém os seus (c0cff4d vs 32283d7; libcosmic diferentes) | o crate passa a ser compilado com o lock de cada consumidor; qualquer upgrade de pin vira trabalho nos dois ao mesmo tempo |
| Acoplamento | — | o handler de screencopy é `impl ScreencopyHandler for <estado concreto>` + macros `delegate_*`; o cartão é `Element<Message>` da barra ⇒ o crate precisaria de genéricos/macros para os dois `Message` e os dois estados |
| Correções | a cópia já nasce com timeout/`stopped`/sem `unwrap` (defeitos acima) | corrigiria a barra junto (bom), mas mistura duas entregas |
| Manutenção | duas cópias de ~300 linhas que mudam raramente (protocolo estável) | uma cópia |

"Passa a fazer parte do applet do menu iniciar" = a prévia passa a existir **também** no menu. Registrar
como pendência (não nesta entrega): quando os dois repos estiverem no mesmo pin de epoch (1.10), extrair
`cosmic-ext-window-preview` (captura + `shrink` + cartão genérico) e levar as correções de timeout para a barra.
Cabeçalho de cópia: manter `Copyright 2023 System76 <info@system76.com>, 2026 Aenes0` + `GPL-3.0-only` (mesma licença).

### D2 — Agrupamento em Abertos

- **Chave**: `app_for_window(w).map(|a| a.id.clone())`; senão `w.app_id.to_lowercase()`; se `app_id`
  vazio ⇒ `"window:" + w.id` (grupo de uma janela só, nunca junta desconhecidos).
- **Ordem dos grupos**: pela primeira aparição na lista `self.windows` (que já vem por ordem de abertura);
  dentro do grupo, a ordem de `self.windows`. Estável quando títulos mudam.
- **Ícone do grupo**: o mesmo `open_icon` atual (36·1.15 px, `CELL` 65), id `scoped("open", key)`.
  Realce (`flat_class(p, true)`) se **qualquer** janela do grupo está ativa.
- **Selo de contagem** quando `n > 1`: pílula no canto inferior direito do ícone, texto `n` (`"9+"` acima
  de 9), fundo `p.accent`, texto branco, tamanho 11 (mesmo estilo de `Self::badge`). `n == 1` sem selo.
- **Dica (tooltip)**: removida do ícone de grupo — a prévia mostra títulos (a barra só mostra dica em app
  sem janela). Evita dica (500 ms) e prévia (400 ms) brigando.
- **Clique esquerdo**: `n == 1` ⇒ `Message::Activate(id)` (como hoje: ativa e esconde o menu).
  `n > 1` ⇒ abre a prévia **na hora** (sem o atraso), como a barra/Windows; se já está aberta para esse
  grupo, nada. A escolha da janela é feita na prévia.
- **Clique direito**: `Target::OpenGroup(key)` (novo) ⇒ cartão ancorado com título = nome do app (ou
  `app_id`), itens: "Alternar para" (`Activate` da janela ativa do grupo, senão a última = mais nova),
  "Nova janela" (se `app_for_window` conhecido ⇒ `Launch`), "Fechar janela" (`n == 1`) ou
  "Fechar todas as janelas" (`n > 1` ⇒ `Command::Close` para cada). `Target::Window` continua existindo
  para os resultados da busca (que seguem por janela). Abrir o menu de contexto ⇒ prévia `Idle` (I3).
- **Arrastar**: a origem continua `drag::Source::App(app.id)` quando o app é conhecido ⇒ soltar nos Fixados
  = fixar o app (comportamento atual, agora 1 vez por grupo). `DragArm` ⇒ prévia `Idle`.
- **Paginação**: `open_strip.rs` **não muda** (as funções já são genéricas em `total`); `menu.rs` passa a
  chamar com `groups.len()`. `adjust_open(prev_groups, groups, active_group_index, changed, offset)` com
  `changed` = **a chave do grupo ativo** mudou (alternar entre duas janelas do mesmo app não pula a página).
  `open_active` passa a guardar a chave do grupo, não o id da janela.
- **Cache**: `open_groups: Vec<OpenGroup>` recalculado em `Message::Windows` **e** em `Message::Apps`
  (a chave depende de `app_for_window`). A view só lê.

### D3 — Prévia ao passar o mouse

- **Atraso**: abrir **400 ms** após entrar no ícone; fechar **300 ms** após o ponteiro sair do ícone e da
  prévia; trocar de ícone com a prévia aberta ⇒ troca **na hora** (igual à barra).
- **Conteúdo**: igual à barra — cartão 224×164, cabeçalho (ícone do app 16, título 1 linha com
  reticências — vazio ⇒ `texts().untitled` (novo), botão fechar `window-close-symbolic` 16 só com o
  cartão em hover, espaço de 24 px senão), imagem `Contain` com cantos `radius_s`; sem imagem (ainda
  não chegou, falhou, minimizada) ⇒ ícone do app 48 centralizado. Cartão da janela ativa `selected`.
  Clique no cartão ⇒ `Message::Activate(id)` (ativa, esconde o menu, prévia some com o `hide`).
  Botão do meio ⇒ fecha a janela. Botão fechar ⇒ `Command::Close(id)`, **prévia continua** (o grupo
  se atualiza pelo próximo `Message::Windows`; grupo vazio ⇒ `Idle`).
- **Várias janelas**: lado a lado, até `cols = min(n, 7, fit(bounds.width - 2·PAD, 224, 8))` por linha,
  `rows = ceil(n / cols)`; se a altura passar de `bounds.height`, a grade fica num `scrollable` vertical
  de altura `bounds.height`. `PAD = 8`, `GAP = 8` (os da barra).
- **Posição** (pura, `preview::place(anchor, size, bounds) -> Point`): `anchor` = retângulo do ícone
  medido com `anchor::find(scoped("open", key))` (coordenadas da superfície, já reflete a largura e a
  posição **dinâmicas** do menu — nada usa `WIDTH`/`INNER`); `bounds` = superfície
  `(0, 0, room_width, room)` encolhida por `MARGIN`. Vertical: **acima** do ícone
  (`anchor.y - GAP - h`) se cabe em `bounds`; senão **abaixo** (`anchor.bottom + GAP`) se cabe; senão
  `bounds.bottom - h` limitado a `bounds.y`. Horizontal: centrada no ícone, presa em
  `[bounds.x, bounds.right - w]`. A prévia pode passar das bordas do menu (como na barra), nunca da tela.
  Compatível com o redimensionamento 400..874 em paralelo: só depende de `anchor` e da superfície.
- **Camada**: em `view_window`, depois de `placed` e antes de `anchored_popup`/`ghost`:
  `pin(mouse_area(card).on_press(Message::Nothing))` — clique na prévia não cai no `backdrop` (que
  esconderia o menu).
- **Saída**: a prévia fecha (via `Leaving` → `Idle`) quando o último ponto do ponteiro não está em
  `anchor ∪ card ∪ ponte`, onde `ponte` = retângulo entre ícone e prévia (largura do ícone, altura `GAP`),
  e em `CursorLeft` da superfície. Fecha **na hora** (`Close`) em: `hide()`, Escape (prioridade: prévia
  antes do menu de contexto), `Query`/`TypeAhead`, `Page`, `DragArm`, `Menu`/`MenuFor`/`MenuAt`,
  `AskUninstall`, rolagem (`WheelScrolled` enquanto não-`Idle`: o ícone mudou de lugar), grupo sumiu.

#### Máquina de estados (`src/menu/preview.rs`, pura)

```rust
pub enum Phase { Idle, Waiting { key: String }, Shown { key: String }, Leaving { key: String } }
pub struct Hover { phase: Phase, gen: u64, anchor: Option<Rectangle>, card: Option<Rectangle>, pointer: Option<Point> }
pub enum Event {
    EnterIcon(String),          // on_enter do ícone do grupo (pode vir 2×)
    LeaveIcon(String),          // on_exit do ícone (dica; a geometria manda)
    ClickGroup(String),         // n > 1: abre já
    Pointer(Point),             // CursorMoved na superfície do menu
    PointerGone,                // CursorLeft
    Anchor(u64, Option<Rectangle>), // resposta de anchor::find para a geração
    Card(Rectangle),            // retângulo da prévia calculado por place()
    Timer(u64),                 // fim de atraso de abrir/fechar
    Close,                      // hide/Escape/busca/página/arrasto/menu/rolagem/grupo vazio
}
pub enum Effect { Wait(Duration, u64), Measure(String, u64), StartCapture(String), StopCapture }
pub fn step(h: &mut Hover, ev: Event, blocked: bool, exists: impl Fn(&str) -> bool) -> Vec<Effect>;
```

Transições (toda mudança de alvo/fase faz `gen += 1`; timer/anchor com `gen` antiga é ignorado):

| De | Evento | Para | Efeitos |
|---|---|---|---|
| `Idle` | `EnterIcon(k)` | `Waiting{k}` | `Measure(k,g)`, `Wait(400,g)` |
| `Waiting{k}` | `EnterIcon(k)` (duplicado) | igual | nenhum (I5) |
| `Waiting{k}` | `EnterIcon(j≠k)` | `Waiting{j}` | `Measure(j,g)`, `Wait(400,g)` |
| `Waiting{k}` | `LeaveIcon(k)` / `Pointer` fora / `PointerGone` | `Idle` | — |
| `Waiting{k}` | `Timer(g)` | `blocked`∨`!exists(k)`∨`anchor=None` ⇒ `Idle`; senão `Shown{k}` | `StartCapture(k)` se `Shown` (I1, I3) |
| `Idle`/`Waiting`/`Leaving` | `ClickGroup(k)` | `Shown{k}` (se `!blocked`∧`exists`) | `Measure(k,g)`, `StartCapture(k)` |
| `Shown{k}` | `EnterIcon(j≠k)` | `Shown{j}` | `Measure(j,g)`, `StartCapture(j)` (troca na hora) |
| `Shown{k}` | `EnterIcon(k)` / `Pointer` dentro | igual | — |
| `Shown{k}` | `Pointer` fora / `PointerGone` / `LeaveIcon(k)` com ponteiro fora | `Leaving{k}` | `Wait(300,g)` |
| `Leaving{k}` | `Pointer` dentro / `EnterIcon(k)` | `Shown{k}` | — (gen++ cancela o timer) |
| `Leaving{k}` | `Timer(g)` | ponteiro dentro ∧ `!blocked` ∧ `exists` ⇒ `Shown{k}`; senão `Idle` | `StopCapture` se `Idle` (I1) |
| qualquer | `Close` | `Idle` | `StopCapture` se saiu de `Shown`/`Leaving` |
| qualquer | `Anchor(g', r)` com `g'≠g` | igual | — |

"Dentro" = `pointer ∈ anchor ∪ card ∪ ponte`; enquanto `anchor` não chegou, só `EnterIcon/LeaveIcon` valem.
`StartCapture`/`StopCapture` só ligam/desligam a assinatura de atualização e as fotos (D4).

### D4 — Captura no menu

- **Mesmo loop Wayland** do `run_windows` (não um segundo): o `State` ganha `capture: capture::CaptureState`
  (`ScreencopyState::new(&globals,&qh)` + `Shm::bind(&globals,&qh)`), e `conn`/`qh` guardados. A conexão é
  a mesma que já dá os `ExtForeignToplevelHandleV1` — o handle de captura tem de ser do **mesmo** `Connection`.
  `Shm::bind` falhar ⇒ captura desligada (log `warn`), lista de janelas segue funcionando.
- **Pedido**: `Command::Capture(Vec<String>)` (ids). A thread acha o `ToplevelInfo` pelo `identifier`,
  pula se `!WINDOWS_SHOWN`, e dispara uma thread curta por janela (modelo da barra) que roda
  `capture_source_shm_fd` + `shrink(…, 400)` e manda `Message::Thumb(id, Option<Handle>)` pelo `tx`.
- **Correções sobre a cópia** (obrigatórias): nenhuma `unwrap()`/`expect()` (erro ⇒ `Thumb(id, None)`);
  `wait_while` vira `Condvar::wait_timeout_while` com **2 s**; `stopped` marca a sessão como falha; aceita
  `Abgr8888` **ou** `Xbgr8888` (no X, alfa forçado a 255); `CaptureSession`/pool/buffer destruídos em
  todos os caminhos; nenhuma foto pedida para janela sem handle. **Toda** foto pedida gera exatamente uma
  `Thumb` (sucesso ou `None`) — senão a janela fica "em captura" para sempre (defeito da barra).
- **Quando**: só com a prévia `Shown`/`Leaving` **e** a superfície visível. Ao entrar em `Shown`: pede as
  janelas do grupo; depois `iced::time::every(1000 ms)` (assinatura ligada só nesse estado) pede de novo,
  exceto as que estão em `capturing: HashSet<String>` (em voo) e as minimizadas que já têm foto (como a
  barra). `Window` ganha `minimized: bool` (de `State::Minimized` em `report()`).
- **Liberar**: `StopCapture` ⇒ `thumbs.clear()` e `capturing.clear()` (fotos que chegarem depois de
  `Idle` são descartadas: só se guarda `Thumb` de janela do grupo mostrado). `hide()` já agenda
  `malloc_trim`. Janela que some ⇒ remove a foto.
- **Custo estimado**: por foto, memfd do tamanho da janela (ex.: 3440×1440×4 ≈ 19,8 MB, transitório, liberado
  ao fechar o fd) + ~2–5 ms de CPU para reduzir; miniatura ≤ 400 px ≈ 0,36 MB; 7 janelas ⇒ ~2,5 MB
  residentes enquanto a prévia está aberta, 0 com ela fechada. Upload de textura (wgpu) de cada `Handle`
  novo a 1 Hz. Escondido: zero captura (gate duplo: UI + `WINDOWS_SHOWN` na thread).
- **Pixel**: `Abgr8888` little-endian = bytes `R,G,B,A` ⇒ RGBA direto; `Handle::from_rgba(w, h, Bytes::from_owner(img.into_vec()))`
  (um `Handle` por foto, criado uma vez: recriar a cada quadro reenviaria a textura).
- **Janelas de outro workspace/saída**: o menu lista todas; a captura pode falhar ou vir velha ⇒ ícone 48
  (aceito; a barra filtra pelo workspace e não tem esse caso).

### D5 — Dependências (`start-menu/Cargo.toml`)

Adicionar as mesmas da barra: `image = { version = "0.25.9", default-features = false }`,
`memmap2 = "0.9.10"`, `rustix = { version = "1.1", features = ["fs"] }`. **Pins e `[patch]` não mudam**
(32283d7 já tem o mesmo `screencopy`). Atualizar o comentário dos pins é fora de escopo.

## 2. Arquivos

| Arquivo | Novo? | Conteúdo |
|---|---|---|
| `src/menu/open_groups.rs` | novo | `OpenGroup { key, windows: Vec<usize> }`, `group(windows, key_of) -> Vec<OpenGroup>`, `active_group(groups, windows) -> Option<usize>`, `switch_target(group, windows) -> &str` (ativa senão a última), `count_label(n) -> Option<String>`; testes |
| `src/menu/preview.rs` | novo | `Hover`, `Phase`, `Event`, `Effect`, `step`, `inside`, `place`, `grid(n, bounds) -> (cols, rows, Size)`, constantes (`OPEN 400ms`, `CLOSE 300ms`, `REFRESH 1000ms`, `CARD_W 224`, `CARD_H 164`, `PAD 8`, `GAP 8`, `MAX_COLS 7`); testes |
| `src/menu/capture.rs` | novo | cópia adaptada: `CaptureState`, `Session*`, `FrameData`, `CaptureData`, `ShmImage`, `shrink(raw, w, h, stride, max, opaque) -> RgbaImage` (puro), macro `delegate_capture!($ty)` (impls `ShmHandler`, `ScreencopyHandler`, `Dispatch<WlShmPool>`, `Dispatch<WlBuffer>`, `delegate_shm!`, `delegate_screencopy!`), `fn spawn_capture(...)`; testes de `shrink` e um `#[cfg(test)]` estado fictício que expande a macro |
| `Cargo.toml` (+`Cargo.lock`) | — | D5 |
| `src/texts.rs` | — | `untitled`, `close_all_windows`, (`switch_to`/`new_window`/`close_window` já existem) em pt/en como os demais |
| `src/menu.rs` | — | ganchos (série, ver ondas) |
| `src/menu/open_strip.rs` | — | **não muda** |

## 3. Testes

**Puros (cargo test, TDD):**
- `open_groups`: agrupa 3 janelas do mesmo app + 1 de outro na ordem de aparição; app desconhecido cai em
  `app_id` minúsculo; `app_id` vazio nunca agrupa; grupo ativo; `switch_target` prefere a ativa, senão a
  última; `count_label(1)=None`, `(2)="2"`, `(10)="9+"`.
- `preview::step` — além de cada linha da tabela, **regressões do bug**:
  (a) `LeaveIcon` perdido: `Shown` + `Pointer` fora ⇒ `Leaving`; `Timer(g)` ⇒ `Idle` (sem depender de exit);
  (b) enter/exit duplicados não criam timers extras nem mudam a fase;
  (c) menu de contexto aberto durante `Waiting` ⇒ `Timer(g)` com `blocked` ⇒ `Idle`, e o próximo
  `EnterIcon` abre normalmente (nada "trancado");
  (d) `Close` em qualquer fase ⇒ `Idle` e próximo hover funciona;
  (e) timer/anchor de geração velha ignorado;
  (f) propriedade: para qualquer sequência de eventos gerada (~200 sequências pseudo-aleatórias de um
  LCG semeado escrito no próprio teste — sem dependência nova), depois de
  `Pointer(fora)` + disparar o último `Wait` pendente, a fase é `Idle` (I1).
- `preview::place`: acima quando cabe; abaixo quando não cabe acima; presa nas bordas esquerda/direita;
  superfície menor que a prévia ⇒ origem em `bounds.x/y`; menu 400 e 874 de largura (via `anchor`).
- `preview::grid`: 1, 7, 8, 15 janelas; largura estreita limita colunas; nunca 0 colunas.
- `capture::shrink`: 4×4 → 2 px de lado mantém a média; `opaque` força A=255; `stride` > `w·4`; tamanho
  errado ⇒ `Err`, nunca pânico.
- `menu.rs`: `adjust_open` com contagem de grupos (2 janelas do mesmo app ⇒ 1 posição).

**Só na tela (o ambiente não tem captura de tela confiável nem injeção de input — quem testa é o usuário):**
captura real (miniaturas aparecem e atualizam a 1 Hz), sensação dos atrasos, prévia não fica presa
(passar rápido entre ícones, sair rápido para fora da tela, abrir menu de contexto durante o atraso,
Escape, rolar), posição acima/abaixo com painel embaixo e em cima, menu estreito (400) e largo (874),
minimizadas e de outro workspace (ícone), fechar janela pela prévia, CPU/memória com a prévia aberta
(`top -p $(pidof cosmic-ext-applet-start)`). Para ver sem passar o mouse: `COSMIC_START_PREVIEW=windows:<app_id>`
abre a prévia desse grupo uma vez (como `COSMIC_TASKBAR_PREVIEW`).

## 4. Ondas (operários Sonnet low; arquivos disjuntos; TDD; **não commitar**)

```
Onda 1 (paralela) ── T1 open_groups.rs ─┐
                  ── T2 preview.rs ─────┤
                  ── T3 capture.rs + Cargo.toml/.lock ─┤
                  ── T4 texts.rs (haiku) ┘
Onda 2 (série, só menu.rs) ── T5 thread: Window.minimized, State.capture, delegate_capture!,
                                Command::Capture, Message::Thumb  (dep. T3)
                           └─ T6 Abertos por grupo: cache open_groups, open_icon→ícone de grupo
                                com selo, clique/clique direito (Target::OpenGroup), paginação por
                                grupo, adjust_open por chave  (dep. T1, T4)
                           └─ T7 prévia: campos Hover/thumbs/capturing, mensagens, executor de
                                Effect, assinaturas (CursorMoved/CursorLeft/WheelScrolled só fora de
                                Idle; every(1s) só em Shown/Leaving), camada na view, Close nos pontos
                                de D3, COSMIC_START_PREVIEW=windows:  (dep. T2, T5, T6)
Onda 3 ── G3 (cargo test + clippy + build release via flatpak-spawn --host) → revisor → usuário testa
```

Brief de cada operário: objetivo, arquivos permitidos, assinaturas reais acima, "não commite",
**licença de corrigir o plano** (se o brief não compilar contra as assinaturas reais, corrija e relate;
nunca afrouxe teste), verificação `cargo test --manifest-path ~/Projetos/start-menu/Cargo.toml <módulo>`.
`menu.rs` sempre em série — e **coordenar com o redimensionamento (400..874) e as outras frentes que
mexem em `menu.rs`/`texts.rs`/`Cargo.toml`**: a onda 2 desta spec entra depois que essas fatias
fecharem, ou numa worktree separada com merge manual.

## 5. Riscos

| Risco | Mitigação |
|---|---|
| Prévia presa (bug da barra) | I1–I5; máquina pura com testes de regressão; sem xdg_popup; `hide()` zera |
| `unwrap` numa thread de captura com `panic = "abort"` derruba o menu | proibido `unwrap/expect` em `capture.rs` (revisor confere com `grep`) |
| Thread de captura bloqueada para sempre | timeout 2 s + `stopped` ⇒ falha; toda foto gera uma `Thumb` |
| Custo de captura (memfd do tamanho da janela, 1 Hz × n janelas) | só com prévia aberta e menu visível; em voo não repete; minimizada com foto não repete; `clear` + `malloc_trim` ao fechar |
| Global de captura ausente (outro compositor/sandbox) | `CaptureState` opcional; sem foto ⇒ ícone; lista de janelas intacta |
| Conflito de `menu.rs` com o redimensionamento e demais frentes | onda 2 em série depois delas; prévia não usa `WIDTH`/`INNER` |
| Rolagem move o ícone sob a prévia | `WheelScrolled` ⇒ `Close` |
| `CursorMoved` em excesso | assinatura só fora de `Idle`; `step` é O(1) |
| Pins divergentes (1.8 vs 1.10) | API de screencopy idêntica entre 32283d7 e c0cff4d (verificado); sem mudança de pin |

## 6. Fora de escopo

Mexer na barra de tarefas (inclusive corrigir o bug dela); crate compartilhado; prévia nos resultados da
busca; teclado dentro da prévia; arrastar janelas da prévia; animações; atualizar o comentário dos pins.

## Ajustes da revisão Opus (2026-10-22) — OBRIGATÓRIOS (relatados por agente; conferir no código)
1. **HIGH:** `Event::Card(Rectangle)` não tem emissor (`place()` roda na view). Remover `Card`; `step` recebe `{n, bounds}` e `inside()` calcula `place(anchor, grid(n, bounds), bounds)`. Teste: Shown + ponteiro no centro do cartão ⇒ continua Shown.
2. **MEDIUM:** `Anchor(g, None)` em Shown/Leaving ⇒ Idle + `StopCapture`. Fechar também em `OpenPage`, `ResizeStart`, `Tools`. Em `Windows`/`Resized` com a prévia Shown, refazer o `Measure`.
3. **MEDIUM:** `DragArm ⇒ Close` faria o clique no ícone (que passa por `DragArm`) fechar e reabrir a prévia: fechar SÓ quando o arrasto vira `Dragging`.
4. **MEDIUM:** clique direito na prévia, fora do menu, chega ao backdrop e esconde o menu: o mouse_area do cartão usa `.on_right_press(Nothing).interaction(Interaction::Idle)`.
5. **MEDIUM (captura):** `create_session` do cctk tem `.expect` (com `panic=abort` derruba o menu): ligar a captura SÓ se existirem os dois globais ext (`ext_image_copy_capture_manager_v1` e `ext_foreign_toplevel_image_capture_source_manager_v1`); `Shm` obrigatório (`ShmHandler::shm_state()` exige `&mut Shm`).
6. **MEDIUM (custo):** não uma thread + memfd por janela: UM worker de captura em série, reutilizando um memfd (`ftruncate` checado, `checked_mul`, teto de 64 MB). Pico real de n×20 MB, não 2,5 MB. Um `ftruncate` ignorado ou `w*h*4 as i32` sem checagem derruba a conexão compartilhada (e a lista de janelas).
7. **LOW:** `WheelScrolled ⇒ Close` só se o ponteiro estiver FORA do cartão; pedido pulado (`!WINDOWS_SHOWN`) ou janela sem handle ⇒ `Thumb(None)`; `Target::OpenGroup` entra no `matches!` de `anchored()`; chaves com prefixo (`app:`, `id:`, `window:`).
Ordem única do stack raiz: `[backdrop, placed, alças, prévia (slot fixo), anchored_popup?, unpin_bar?, ghost?, capa?]`.
