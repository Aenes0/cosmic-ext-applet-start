# Grupos de apps: arrastar dentro/para dentro/para fora, fixar grupos, popup denso (spec + plano)

Pedido (2026-10-22): reordenar arrastando **dentro** do grupo; arrastar **para dentro/fora** põe/tira o
app; grupos **fixáveis**; grupos com **densidade maior** (inspiração One UI 8). Branch `main`, sem commit.

Decisões do usuário: (1) grupo = pasta de **categoria** (`Target::Category` de hoje) **ou** grupo do
**usuário** (app solto sobre app cria; renomear; apagar); todos aceitam arrastar e podem ser fixados.
(2) Arrastar **para fora** ⇒ app fica **sem grupo**; pasta automática **"Sem grupo"** visível quando não
vazia. (3) Grupo fixado = tile de pasta com **mini-ícones**; clique abre o popup. (4) Popup com **6 por
linha**, tiles compactos. (5) `main`, sem commit.

Lições herdadas (spec `2026-10-22-menu-fixados-abertos.md`): o botão do libcosmic captura o clique
esquerdo (armar só com `on_press_down_with_rectange`, nunca `mouse_area.on_press`); `container::visible_bounds`
do iced recursa sem fim — usar `anchor::*`; subscription de arrasto só fora de `Idle`; a árvore de widgets
não muda de estrutura condicionalmente (overlays sempre presentes).

## 1. Modelo de dados puro — `src/menu/groups.rs` (novo, sem iced)

```rust
pub const CATEGORIES: usize = 10;            // == texts().categories.len() (teste em menu.rs)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupRef { Category(usize), User(u32), Loose }   // Copy: mantém drag::Slot Copy
#[derive(Clone, Copy, Debug)]
pub struct AppKey<'a> { pub id: &'a str, pub category: usize } // um por AppEntry, mesmo índice
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Groups {
    pub user: Vec<(u32, String)>,              // grupos do usuário (id estável, nome), ordem de criação
    pub place: Vec<(String, GroupRef)>,        // SÓ exceções: app id -> grupo (≠ categoria padrão)
    pub order: Vec<(GroupRef, Vec<String>)>,   // ordem explícita por grupo (parcial)
}
```
- **Pertença única por construção:** a pertença sai de `place` (um registro por app) ou, sem registro, de
  `Category(app.category)`. Um app nunca está em dois grupos. `order` é só cosmética (ids que não são
  membros são ignorados).
- **Chaves:** `GroupRef::key()` → `"cat:3"`, `"usr:7"`, `"none"`; `parse(&str) -> Option<GroupRef>`
  rejeita `cat:N` com `N >= CATEGORIES`, número inválido, vazio. Fixado: `pin() -> Option<String>` =
  `"group:" + key` (`Loose` → `None`, não fixável); `from_pin(&str) -> Option<GroupRef>`.
- **Ids de app:** sempre o `AppEntry.id` exato (o arrasto vem de tiles com o id real); nada de
  `find_app` aproximado aqui.

Funções (todas puras, `&self` → novo `Groups`; imutável):
| Assinatura | Regra |
|---|---|
| `from_stored(user: Vec<(u32,String)>, place: Vec<(String,String)>, order: Vec<(String,Vec<String>)>) -> Groups` | normaliza: id de usuário duplicado → fica o 1º; chave inválida → descartada; `place` para `User(id)` inexistente → descartado; app repetido em `place` → fica o 1º; `place` igual à própria categoria só é removido em `place()` (não dá para saber a categoria aqui); `order` com grupo repetido → 1º; ids repetidos numa lista → 1º |
| `to_stored(&self) -> (…mesmos 3 tipos…)` | inverso; `from_stored(to_stored(g)) == g` |
| `group_of(&self, app: AppKey) -> GroupRef` | `place` ou `Category(app.category)` |
| `members(&self, g, apps: &[AppKey]) -> Vec<usize>` | índices em `apps` dos membros: primeiro na ordem de `order[g]`, depois os demais na ordem de `apps` (já alfabética) |
| `visible(&self, apps) -> Vec<(GroupRef, Vec<usize>)>` | grupos do usuário (ordem de criação), categorias 0..10, `Loose` por último; **só os não vazios** |
| `exists(&self, g) -> bool` | `User(id)` em `user`; `Category(n<10)`; `Loose` sempre |
| `name(&self, id: u32) -> Option<&str>` | — |
| `place(&self, app, into, before: Option<usize>, apps) -> Groups` | move/reordena: `cur = members(into)`; se `app` já está em `cur` na posição `f`, sai; destino `before.map(|i| if mesmo grupo && i > f { i-1 } else { i })`, `None` = fim, limitado a `len`; grava `order[into]` com a lista completa; tira `app` das outras listas de `order`; `place[app] = into`, **ou remove o registro** se `into == Category(app.category)`; `into` inexistente → inalterado |
| `merge(&self, app, onto, name: &str, apps) -> (Groups, GroupRef)` | `app.id == onto.id` → inalterado, devolve `group_of(onto)`; `onto` já num `User(u)` → `place(app, User(u), Some(pos(onto)+1))`; senão cria `id = max(ids)+1` (ou 1), push `(id, name)`, `place(onto, User(id), None)` e `place(app, User(id), None)` |
| `rename(&self, id, name) -> Groups` | grava `name.trim()`; vazio → inalterado |
| `delete(&self, id) -> Groups` | remove o grupo, os `place` que apontam para ele (apps **voltam à categoria padrão**) e seu `order` |
| `forget(&self, app_id) -> Groups` | remove `place` e o id de todas as listas de `order` (app desinstalado) |
| `emptied(&self) -> (Groups, Vec<GroupRef>)` | remove grupos do usuário **sem nenhum registro em `place`** e devolve quais; vazio é medido por `place`, não pelos apps instalados (apps ausentes temporariamente não apagam grupo; e nunca é chamado com `apps` vazio) |
| `next_name(&self, base: &str) -> String` | `base`, ou `base 2`, `base 3`… o primeiro livre |
| `unpin(pinned: &[String], gone: &[GroupRef]) -> Vec<String>` (livre) | tira as entradas `group:…` dos grupos removidos |

**Persistência** (cosmic config `APP_ID` v1, como `pinned`): chaves novas `groups: Vec<(u32, String)>`,
`group_of: Vec<(String, String)>`, `group_order: Vec<(String, Vec<String>)>` (tuplas: sem `serde` derive,
sem dependência nova). Ausentes ⇒ `Groups::default()` (**sem migração**). `load_groups()`/`save_groups()`
em `menu.rs` ao lado de `load_pinned`/`save_pinned`. Downgrade é seguro: o binário antigo ignora entradas
`group:` em `pinned` (`find_app` não as resolve).

## 2. `self.pinned` com grupos

`self.pinned: Vec<String>` continua; grupos entram como `"group:cat:3"` / `"group:usr:7"`.
- **`pinned_apps` vira `pinned_items(&self) -> Vec<(usize, PinnedItem<'_>)>`** com
  `enum PinnedItem<'a> { App(&'a AppEntry), Group(GroupRef, Vec<&'a AppEntry>) }`: entrada com
  `GroupRef::from_pin` → grupo (só se `exists` e com ≥1 membro); senão `find_app` como hoje. Os índices
  continuam sendo de `self.pinned` (`visible_pinned`, `area_slot`, `pin:<i>` inalterados).
- **`find_app`/`same_app` inalterados.** `"group:cat:3"` não casa com app (nem exato, nem `loosely`).
- **`drag::apply` inalterado no comportamento de Reorder/Pin**; o `same` passado em `drag_released` passa a
  ser `|item, x| item == x || same_app(apps, item, x)` — a igualdade exata impede fixar o mesmo grupo duas
  vezes (hoje `same_app` daria falso para grupos ⇒ duplicaria).
- `TogglePin(String)` serve a grupos com a chave `group:…` (comparação exata já existente).
- Grupo do usuário apagado/esvaziado ⇒ `groups::unpin`. Categoria fixada que ficou vazia: some dos
  Fixados mas **permanece** em `pinned` (volta se ganhar apps), como app fixado ausente hoje.

## 3. Arrasto — mudanças exatas em `src/menu/drag.rs`

```rust
pub enum Source { Pinned(usize), App(String), Member(GroupRef, String), Group(GroupRef) }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot {
    Before(usize), End,              // Fixados (como hoje)
    Into(GroupRef),                  // cartão de pasta em Todos; centro de grupo fixado
    OnApp(usize),                    // centro de um tile de app: índice em self.apps
    Member(GroupRef, usize),         // antes do membro i no popup aberto
    MemberEnd(GroupRef),             // área (cartão) do popup
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer { PopupCenter, PopupTile, PopupArea, Center, Tile, Area } // menor = vence
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Target { pub slot: Slot, pub rect: Rectangle, pub layer: Layer }
#[derive(Clone, Copy, Debug, PartialEq)] pub enum Zone { Whole, Center }
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Spec { pub slot: Slot, pub layer: Layer, pub zone: Zone }
pub enum Item { App(String), Pinned(usize) }
pub enum Action {
    Reorder { from: usize, to: Slot }, Pin { app: String, to: Slot },        // como hoje
    Place { item: Item, into: GroupRef, before: Option<usize> },
    Merge { item: Item, onto: usize },                                      // onto: índice em self.apps
    Ungroup { app: String },
    Cancel,
}
```
- `moved(point, &[Target])` e `target_at(point, &[Target]) -> Option<Slot>` (primeiro que contém; a lista
  vem ordenada por `layer`). `center(rect) -> Rectangle`: miolo de 50% (margem de 25% em cada lado).
  `targets(specs: &[Vec<Spec>], rects: &[Option<Rectangle>]) -> Vec<Target>`: zipa (um id pode gerar 2
  alvos: borda e centro), aplica `Zone`, descarta `None`, **ordena estável por `layer`** (resolve a
  corrida das respostas assíncronas que hoje o `insert(0)` resolve). `left_popup(point, &[Target]) -> bool`
  = há alvos de popup e nenhum contém o ponto; `without_popup(&[Target]) -> Vec<Target>`;
  `Slot::in_popup()` = `Member | MemberEnd` (+ alvos de camada `Popup*`).
- `wants(source: &Source, pinned_is_group: bool) -> Wants { pinned_centers, folders, tiles, popup }`:
  `Group(_)` e `Pinned` de grupo → só Fixados (borda + área; sem centros, sem pastas, sem tiles);
  `App`/`Pinned` de app → tudo menos popup; `Member` → tudo + popup.
- **`released()` (tabela):**

| origem \ sobre | `Before`/`End` | `Into(g)` | `OnApp(b)` | `Member(g,i)` | `MemberEnd(g)` | nenhum |
|---|---|---|---|---|---|---|
| `Pinned(i)` | `Reorder` | `Place{Pinned(i), g, None}` | `Merge{Pinned(i), b}` | Cancel | Cancel | Cancel |
| `App(a)` | `Pin{a}` | `Place{App(a), g, None}` | `Merge{App(a), b}` | `Place{App(a), g, Some(i)}` | `Place{…, None}` | Cancel |
| `Member(_, a)` | `Pin{a}` (continua no grupo) | `Place{App(a), g, None}` | `Merge{App(a), b}` | `Place{App(a), g, Some(i)}` | `Place{App(a), g, None}` | **`Ungroup{a}`** |
| `Group(g)` | `Pin{g.pin()}` | Cancel | Cancel | Cancel | Cancel | Cancel |

- `apply(pinned, action, same)`: Reorder/Pin como hoje; demais ações → `pinned.to_vec()`.
- `absorb(pinned: &[String], removed: Option<usize>, replaced: Option<(usize, &str)>) -> Vec<String>`:
  tira `removed` (o fixado absorvido pelo grupo) e troca `replaced` pela chave do grupo (ajustando o índice
  se `removed < replaced`); se a chave já estiver em `pinned`, remove a entrada trocada em vez de duplicar.

**Semântica de soltar** (em `menu.rs`, `drag_released`): `Place`/`Merge` com `Item::Pinned(i)` cuja entrada
é grupo → Cancel (sem pastas aninhadas). App fixado arrastado para dentro de um grupo **sai dos Fixados**
(absorvido, como na tela inicial da One UI). `Merge` sobre app **fixado** `b` ⇒ a entrada de `b` vira o
grupo novo. Depois de `Place/Merge/Ungroup`: `emptied()` + `unpin` + `save_groups` + `save_pinned`.
`Merge` que cria grupo abre o popup dele com o nome em edição e selecionado.

## 4. UI — `src/menu/group_ui.rs` (novo) + ganchos em `menu.rs`

- **`Target::Category(usize)` → `Target::Group(GroupRef)`** (popup centrado, fundo escurecido, como hoje)
  e **novo `Target::GroupMenu(GroupRef)`** (menu de contexto, ancorado: entra em `anchored()`).
- **Popup do grupo (`popup_body`)**: largura `POPUP_WIDTH = 640`, `POPUP_COLUMNS = 6`, cantos 16 (o
  cartão do popup ganha `.id(POPUP_CARD)` = `"group-popup"`). Título centrado (`heading`), editável para
  grupo do usuário (ver §5). Grade rolável (`max_height = HEIGHT*0.6`) de `member_tile`: ícone `px(32)`,
  nome `ellipsize(_, 11)` numa linha, `padding [10,2,6,2]`, espaçamento 6 (colunas) / 10 (linhas). Cada
  tile: id `grp:<key>:<i>` (`member_id`), `draggable(…, Source::Member(g, id), Launch(id), ativo)`,
  clique direito `MenuFor(Target::App(id), …)`, overlay sempre presente (esmaecido se arrastado; barra
  de destaque se `over == Member(g,i)`; anel se `over == OnApp(idx)`). O popup **não** usa mais
  `tile_grid(.., 5, ..)` (elimina também ids `all:<id>` duplicados entre popup e grade).
- **Tile fixado de grupo**: quadrado `PIN_BOX = 52` com `folder_class`, raio 14, `padding [8,0]` (altura
  da linha igual à dos apps: 52+16 ≈ 41+28); mini-ícones pela regra `mini_grid(n)`: **n ≤ 4 → 2×2
  (até 4); n ≥ 5 → 3×3 com os 9 primeiros**; `mini_metrics(cols, box, pad, gap) -> u16` =
  `floor((box - 2·pad - (cols-1)·gap)/cols)`: Fixados `pad 5, gap 3` → **19 px (2×2) / 12 px (3×3)**.
  Mesmo `pin:<i>`, `Source::Pinned(i)`, tooltip com o nome, clique abre `Target::Group(g)`, direito
  `MenuFor(GroupMenu(g), pin:<i>)`.
- **Cartão de pasta em Todos (vista Categoria)**: `categories()` passa a iterar `groups.visible(..)`
  (usuário, categorias, "Sem grupo"); cartão de 144 px como hoje, id `fold:<key>`, mini-ícones
  `FOLDER_BOX = 104, pad 6, gap 14` → **39 px (2×2) / 21 px (3×3)**; arrastável como `Source::Group(g)`
  se `pinnable` (não "Sem grupo"); direito → `GroupMenu(g)`; anel quando `over == Into(g)`.
- **Fantasma**: `Source::Group`/`Pinned` de grupo → `mini_folder`; demais → ícone (como hoje).
- **Textos novos** (`texts.rs`, PT/EN): `ungrouped` "Sem grupo"/"Ungrouped", `new_group` "Novo
  grupo"/"New group", `group_name` "Nome do grupo"/"Group name", `rename` "Renomear"/"Rename",
  `delete_group` "Apagar grupo"/"Delete group". Nome exibido: usuário → `name`; categoria →
  `texts().categories[n]`; `Loose` → `ungrouped`.

## 5. Criar, renomear, apagar, fixar

- **Criar:** soltar app no **centro** de outro app (tile da grade/linha da lista de Todos, tile fixado de
  app, ou membro de popup de **categoria/"Sem grupo"**; dentro de grupo do usuário não há centro: sem
  aninhar) ⇒ `merge` com `next_name(texts().new_group)`.
- **Menu de contexto `GroupMenu(g)`:** "Abrir" (`Menu(Some(Group(g)))`); "Renomear" (só usuário);
  "Fixar no Iniciar"/"Desafixar do Iniciar" (`TogglePin(g.pin())`, se `pinnable`); "Apagar grupo" (só
  usuário; apps voltam à **categoria padrão**; sem confirmação — nada é perdido além do nome).
- **Nome:** o título do popup de grupo do usuário é um `text_input` (id `GROUP_NAME`, sem borda, estilo de
  título, placeholder `group_name`). Estado `group_edit: Option<(u32, String)>` (rascunho).
  `GroupName(u32, String)` atualiza o rascunho (sem trim: espaços digitáveis); `GroupNameDone`
  (on_submit) confirma. **Toda saída do popup confirma** via `close_menu()` (novo helper que substitui os
  pares `self.menu = None; self.menu_anchor = None;` de `update`/`show`/`uninstalled`): `rename` (trim;
  vazio mantém o nome) + `save_groups`. `RenameGroup(u32)` abre o popup e devolve
  `text_input::focus(GROUP_NAME)` + `text_input::select_all(GROUP_NAME)` (existem em
  `cosmic::widget::text_input`). Esc fecha o popup confirmando o nome (Esc durante arrasto continua só
  cancelando o arrasto).
- `COSMIC_START_PREVIEW=<n>` → `Target::Group(Category(n))`; novo `group:<key>` → `Target::Group(parse)`.

## 6. Hit-test e o arrasto entre popup e menu

- **Uma travessia só:** `anchor::find_many(ids: Vec<widget::Id>) -> Task<Vec<Option<Rectangle>>>` —
  mesma `Operation` de `find` (mesma lógica de rolagem/recorte), sem parar no primeiro achado. Hoje são N
  travessias (uma por fixado); com tiles da grade (centenas) isso não escala.
- `arm_drag` monta `specs: Vec<Vec<Spec>>` + `ids` conforme `wants(source)`: Fixados `pin:<i>` →
  `Before(i)` `Tile/Whole` (+ centro `OnApp`/`Into` `Center/Center` se `pinned_centers`); área → `End`/
  `area_slot` `Area/Whole`; pastas `fold:<key>` → `Into(g)` `Tile/Whole`; tiles `all:<id>` (grade e lista)
  → `OnApp(idx)` `Tile/Center`; popup: `grp:<key>:<i>` → `Member(g,i)` `PopupTile/Whole` (+ `OnApp`
  `PopupCenter/Center` se `g` é categoria ou `Loose`) e `group-popup` → `MemberEnd(g)` `PopupArea/Whole`.
  `find_many(ids)` → `Message::DragTargets(drag::targets(&specs, &rects))` (substitui `DragTarget`).
  Só retângulos visíveis voltam (rolagem descontada); sem rolagem automática.
- **Sair do popup:** em `drag_moved`, se `Dragging` e `left_popup(point, targets)` ⇒ `close_menu()` e
  `targets = without_popup(..)` **antes** do `moved`. O popup **fecha** e revela Fixados/Todos, cujos
  alvos já foram medidos no armar (o popup é camada sobreposta: não desloca o layout). Não reabre. Soltar
  em lugar sem alvo ⇒ `Ungroup` (Esc cancela). Para **entrar** num grupo: soltar no cartão da pasta em
  Todos ou no centro do grupo fixado (sem "abrir pasta ao pairar" — fora de escopo).
- `Message::Apps` durante arrasto ⇒ `cancel_drag()` (`OnApp(idx)` aponta para `self.apps`).

## 7. Casos de borda (decididos)

Grupo de categoria/"Sem grupo" vazio: some. Grupo do usuário: some e é apagado (e desafixado) quando
`place` não tem mais membros dele; com 1 app continua. Categoria com 1 app: pasta normal. App em dois
grupos: impossível (registro único). App desinstalado: `forget_uninstalled` chama `groups.forget` +
`emptied` + `unpin`. Ids/chaves duplicados ou inválidos: normalizados no `from_stored`. Fixar o mesmo
grupo 2×: igualdade exata no `same`. `Merge` de um app com ele mesmo: nada. Esc: cancela arrasto; fora
dele fecha o popup (confirmando o nome). Grupo arrastado só reordena/fixa (nunca entra em outro).

## Fora de escopo
Abrir pasta ao pairar, rolagem automática, pastas aninhadas, animações, cores de pasta, mostrar grupos
nas vistas Grade/Lista, toque.

## Testes
**Puros (TDD):** `groups.rs` — chaves/parse adversariais (`cat:10`, `cat:-1`, `usr:`, `usr:x`, `group:none`),
roundtrip `to/from_stored`, normalização, `members` (ordem explícita + resto alfabético, ids velhos
ignorados), `place` (reordenar esq./dir./fim no mesmo grupo, mover entre grupos, voltar à própria
categoria remove o registro), **propriedade: após qualquer sequência de place/merge/ungroup/delete cada app
está em exatamente um grupo de `visible`**, `merge` (novo id, alvo já em grupo, mesmo app), `rename`
(trim/vazio), `delete`, `forget`, `emptied`, `next_name`, `unpin`. `drag.rs` — tabela inteira de
`released`, `targets` (ordem por camada independente da ordem de chegada, `Zone::Center`), `center`,
`left_popup`/`without_popup`, `wants`, `absorb` (índices, duplicata), testes antigos preservados.
`anchor.rs` — `find_many` chamando a `Operation` direto (`container`/`scrollable` com stub de
`Scrollable`/`finish`): vários ids, ausente → `None`, rolagem e recorte. `group_ui.rs` — `mini_grid` (0,1,4,5,9,12),
`mini_metrics` (19/12/39/21), `member_id`/`folder_id` distintos por grupo e índice. `menu.rs` —
`CATEGORIES == texts().categories.len()`.
**Não automatizável aqui (sem injeção de input):** os gestos de arrasto, o fechar do popup ao sair, a
precisão do hit-test com rolagem, foco/seleção do nome, dica e anéis — teste manual do usuário; prints
via `COSMIC_START_PREVIEW=group:cat:3` e Fixados com grupo semeado na config.

# Plano de tarefas

Verificação padrão: `flatpak-spawn --host sh -c 'cd ~/Projetos/start-menu && cargo test && cargo build --release'`.
Operários `operario` (sonnet low) com contrato §2.4, "não commite" e licença de corrigir o plano.
`menu.rs` só é tocado por T0, T6, T7, T8 — **em série**. Os demais arquivos são disjuntos.

| # | Onda | Tarefa | Arquivos | Funções / testes | Verificação | Depende |
|---|---|---|---|---|---|---|
| T0 | 0 | **Esqueleto de tipos** (contrato p/ paralelismo): `groups.rs` com `GroupRef`, `AppKey`, `Groups`, `CATEGORIES` e assinaturas da §1 com `unimplemented!()`; `drag.rs` com tipos da §3 (`Source`, `Slot`, `Layer`, `Target`, `Zone`, `Spec`, `Item`, `Action`), `moved/target_at` em `&[Target]`, combinações novas → `Cancel`, testes antigos adaptados; `group_ui.rs` vazio; `menu.rs`: `mod groups; mod group_ui;`, `Target::Group/GroupMenu` (4 usos de `Category` trocados), `Message::{GroupName(u32,String), GroupNameDone, RenameGroup(u32), DeleteGroup(u32)}` com braços vazios, `drag_targets: Vec<drag::Target>` (camada `Area` p/ área, `Tile` p/ tiles), braços de `ghost` p/ fontes novas → `None` | `menu.rs`, `menu/drag.rs`, `menu/groups.rs`, `menu/group_ui.rs` | — (comportamento idêntico) | build + testes atuais verdes | — |
| T1 | 1 | Implementar `groups.rs` (§1) | `menu/groups.rs` | todos os testes de `groups.rs` da seção Testes | `cargo test groups` | T0 |
| T2 | 1 | Implementar `drag.rs` (§3): `released` (tabela), `targets`, `center`, `left_popup`, `without_popup`, `wants`, `absorb`, `apply` | `menu/drag.rs` | testes de `drag.rs` | `cargo test drag` | T0 |
| T3 | 1 | `anchor::find_many` (§6) | `menu/anchor.rs` | testes de `find_many` | `cargo test anchor` | T0 |
| T4 | 1 | Textos PT/EN (§4) | `texts.rs` | — | build | — |
| T5 | 2 | `group_ui.rs`: `mini_grid`, `mini_metrics`, `member_id`, `folder_id`, `POPUP_*`, `GROUP_NAME`, `mini_folder`, `member_tile`, `popup_body(title: Title, members, g, over, dragged, active, p)`, `folder_card`, `drop_ring` | `menu/group_ui.rs` | testes puros de layout/ids | `cargo test group_ui` + build | T1, T4 |
| T6 | 2 | `menu.rs` estado/persistência: `groups`, `group_edit`, `load/save_groups`, `app_keys`, `pinned_items`, `group_name`, `group_apps`, `close_menu` (+ confirma nome), handlers de `GroupName/GroupNameDone/RenameGroup/DeleteGroup`, `GroupMenu` no `popup_card` e em `anchored()`, `same` exato, `forget_uninstalled` estendido, `cancel_drag` em `Apps`, preview `group:<key>` | `menu.rs` | teste `CATEGORIES`; teste do `same` com chave de grupo | build + testes | T1, T4 |
| T7 | 3 | `menu.rs` visual: `categories()` via `visible` + `folder_card`; `popup_card` `Group` via `popup_body` (largura 640, raio 16, id do cartão); tile fixado de grupo no `pinned_grid`; fantasma de grupo; anéis/barras por `over` | `menu.rs` | — | build; prints (preview) | T5, T6 |
| T8 | 4 | `menu.rs` arrasto: `arm_drag` com `wants`+`find_many`+specs → `DragTargets`; `drag_moved` com `left_popup`; `drag_released` despachando `Reorder/Pin` (pinned) e `Place/Merge/Ungroup` (groups + `absorb` + `emptied` + `unpin` + salvar); `Merge` novo abre popup com foco/seleção | `menu.rs` | — | build + testes; teste manual do usuário | T2, T3, T7 |
| T9 | 5 | Revisão (`code-reviewer`/`rust-reviewer` opus) + correções numa onda | — | — | findings tratados | T8 |
| T10 | 5 | Prints (pasta fixada 2×2 e 3×3, popup 6 colunas, "Sem grupo", menu do grupo) | — | — | imagens | T8 |

Ondas: **0** T0 → **1** T1 ∥ T2 ∥ T3 ∥ T4 → **2** T5 ∥ T6 → **3** T7 → **4** T8 → **5** T9 ∥ T10.

Riscos: (1) centro do tile fixado agora cria grupo — reordenar exige soltar na borda (barra vs. anel
deixam claro; ajuste do miolo é uma constante em `center`); (2) `menu.rs` já tem ~3400 linhas — todo
código novo vai para `groups.rs`/`group_ui.rs`, `menu.rs` só recebe ganchos; (3) `find_many` copia a
travessia de `find` — se a lógica de rolagem divergir, os alvos no popup rolado erram (teste com stub);
(4) `text_input` no título rouba/devolve foco da busca (o `TypeAhead` só age com evento `Ignored`);
(5) o arrasto real não tem harness: validação final é manual.

## Ajustes da revisão Opus (2026-10-22) — OBRIGATÓRIOS (relatados por agente; conferir no código ao aplicar)
Veredito do revisor: APROVADO COM AJUSTES. Decisões do usuário NÃO reabertas (grupos de categoria + do usuário; fora do grupo = sem grupo; fixado = mini-ícones One UI; popup 6 por linha).
1. **CRITICAL — `anchor.rs` rolagem em travessia única:** a pilha de rolagem só sai quando aparece um irmão `container` na mesma profundidade (anchor.rs ~l.97-101). A home rolada (menu.rs ~l.1766) é filha única de `container(body)` (~l.865): numa travessia única (`find_many`) o rodapé e todo o popup (`layers`, ~l.870-873) herdam o deslocamento/recorte da home ⇒ retângulos `grp:*`/`group-popup` errados com a home rolada. Correção em `traverse`: depois de `operate(self)`, descartar as rolagens com profundidade ≥ `self.depth - 1`. Teste (T3): rolagem como ÚLTIMO filho seguida de um container com id mais raso.
2. **HIGH — soltar no centro do fixado:** tile ~87 px, vão 12 px, borda ~22 px; a borda DIREITA dá `Before(i)` (app vai para a esquerda de i) e o vão cai em `End`. Corrigir: borda direita = `Before(i+1)`; alvos cobrindo o vão; **`Merge`/`Into` só depois de parar ~450 ms no centro (dwell)**, com o anel de destaque só então; o tempo entra como parâmetro (ms) em funções puras de drag.rs. Alternativa mais segura aceita: nos Fixados o centro só vale para `Into` em grupo JÁ fixado; criar grupo só em Todos ou no popup.
3. **HIGH — sair do popup ⇒ `Ungroup`:** margem de 24–32 px em `left_popup`; `Ungroup` só sobre um alvo "corpo do menu" (`Area`); soltar fora dele (ou sobre o painel) = `Cancel`. `DragMove`/`DragRelease` carregam o `window::Id` e são filtrados no `update` com `accepts_size` (o `listen_with` é `fn` sem captura e ignora a janela, ~l.808).
4. **HIGH — soltar sem teste:** uma função PURA `resolve_drop(action, pinned, groups, keys, find) -> {pinned, groups, open: Option<GroupRef>}` em `src/menu/drop.rs` (onda 1) com TDD: `absorb`, `emptied`+`unpin`, cancelar fixado-de-grupo solto em grupo, qual entrada vira o grupo. Entradas de `self.pinned` podem ser ids soltos ("firefox"): `Item::Pinned(i)` passa por `find_app`; `groups` exige o `AppEntry.id` exato. `drag_released` passa a devolver `Task`.
5. **MEDIUM — categoria por índice:** usar chave estável (`cat:games`…) mapeada para o índice (se `category_of`/`texts.categories` mudarem de ordem nada migra).
6. **MEDIUM — id de grupo:** guardar `next_id` sempre crescente (não `max+1`); `from_stored` descarta `order` de `User` inexistente.
7. **MEDIUM — respostas atrasadas de `DragTargets`:** número de GERAÇÃO (resposta de arme anterior é ignorada). Novos `Slot` quebram os `match` exaustivos em drag.rs (~l.117-121, 138-141): T0 deve deixar o comportamento atual inalterado e testar isso.
8. **MEDIUM — conflito de onda:** constantes/ids (`GROUP_NAME`, `POPUP_*`, `member_id`, `folder_id`) já no esqueleto T0, para T5 e T6 em paralelo não divergirem.
9. **MEDIUM — foco:** `Message::Focused` (~l.616-619) devolve o foco à busca e pode tirá-lo do título; `Message::Query` (~l.665) fecha o menu sem confirmar o nome: entra no `close_menu`.
**MVP (onda A) e depois onda B.** A: grupos de categoria + "Sem grupo"; persistência com chaves estáveis; fixar/desafixar pelo menu e arrastando o cartão da pasta; tile fixado com mini-ícones; popup de 6 colunas com reordenação; arrastar para a pasta/grupo fixado (com dwell); arrastar para fora (com margem). B: grupos do usuário (`Merge` com dwell, `OnApp`, renomear com `text_input`, apagar, itens do `GroupMenu`).
**Ondas ajustadas:** 0 esqueleto ampliado (consts/ids, `Target`, `DragMove` com id da janela) → 1 em paralelo: `groups.rs`, `drag.rs` (+dwell/margem), `drop.rs` (`resolve_drop`), `anchor.rs` (`find_many` + correção da pilha), `texts.rs` → 2 em paralelo: `group_ui.rs` e estado/persistência → 3 `menu.rs` em série (7a popup → 7b tile fixado + fantasma → 7c pastas) → 4 `menu.rs` em série (8a armar com `find_many`/geração/filtro de janela → 8b `drag_moved` com margem → 8c soltar via `resolve_drop`) → 5 revisão + prints + teste manual. T7/T8 usam sonnet **medium** e as subdivisões acima.
**Testes que faltam (obrigatórios):** `find_many` com rolagem como último filho; `resolve_drop`; dwell e margem; geração; `match` de `Slot`. **Só manual:** gestos, fechar o popup, rolagem do popup durante o arrasto, foco do título.

## Correção de spec (2026-10-22, decisão do usuário)
§1 e §5 diziam que, ao APAGAR um grupo do usuário, os apps voltam à categoria padrão. **O usuário decidiu o contrário**: app fora de um grupo "fica sem grupo". Portanto apagar um grupo manda os apps para "Sem grupo" (`Loose`), como `groups.rs::delete` já faz e o teste de `drop/tests.rs` fixa. A spec fica corrigida aqui; o código está certo.
