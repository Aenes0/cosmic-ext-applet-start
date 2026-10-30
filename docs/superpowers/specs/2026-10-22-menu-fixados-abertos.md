# Fixados e Abertos em ícones, arrastar, menu de contexto junto ao item (spec)

Decisões do usuário (2026-10-22): branch `main`, sem commit; "arrastar" = reordenar os fixados **e**
fixar arrastando de fora; Fixados e Abertos **só ícone + dica (tooltip)**; ícones **+15% em todo o menu**.

## 1. Tamanho dos ícones (+15%)
`const ICON_SCALE: f32 = 1.15` e `fn px(base: f32) -> u16` (arredonda). Vale para ícones de **apps e
janelas** (tiles, cards, linhas da lista, resultados e painel de detalhe da busca, pastas da categoria).
Não vale para símbolos de interface (setas, ícones de 16 px dos menus, rodapé).

## 2. Fixados: só ícone + dica
**Widget novo `pinned_tile`** (não alterar `app_tile`: ele também serve a grade de Todos e à pasta de
categoria, que mantêm o rótulo): ícone centralizado (`px(36)`) dentro de `widget::tooltip` com o nome do
app, `Position::Bottom` e **atraso ~500 ms** (`.delay`), para a dica não cobrir alvo e fantasma durante o
arrasto. A estrutura da árvore nunca muda condicionalmente (trocar a estrutura perde estado). Grade e
expandir/recolher (`PINNED_COLUMNS`, `PINNED_ROWS`) inalterados.

## 3. Abertos: faixa cinza acima dos Fixados, só ícones
No topo do `home`, **acima** de Fixados, quando houver janelas: bloco arredondado de tom de cinza (`p.card`,
ou tom derivado se ficar igual ao fundo) com o título "Abertos" e uma linha (com quebra) de ícones das
janelas (`open_icon`, mesmo tooltip com atraso: `título — app`). Clique ativa; clique direito abre o menu
da janela; a janela ativa fica realçada. Some sem janelas. A seção "Abertos" em cards sai.

## 4. Menu de contexto junto ao item
Hoje `popup_menu` centraliza `Target::App/Window/File`. Novo:
- **Âncora por consulta de bounds com `Operation` própria.** `container::visible_bounds` do iced desta
  versão **recursa sem fim** (`container.rs` `traverse` chama `self.traverse`): não usar. Copiar a lógica
  dela para uma `Operation` em `src/menu/anchor.rs` com `traverse` correto (`operate(self)`), disparada por
  `cosmic::iced::runtime::task::widget`. Devolve o retângulo visível do container em coordenadas da
  janela, descontando a rolagem.
- Cada item recebe `widget::Id` único por **escopo**: `pin:<índice em self.pinned>`, `all:<id>`,
  `rec:<id>`, `open:<id da janela>`, `res:<id>` (o mesmo app aparece em várias seções).
- Clique direito → `Message::MenuFor(target, id)` → Task de consulta → `Message::MenuAt(target,
  Option<Rectangle>)`. **`self.menu` só é definido ao chegar `MenuAt`** (sem quadro centralizado antes).
  Sem âncora (consulta falhou) → comportamento atual (centro).
- **Posição (função pura testada):** canto superior esquerdo no canto inferior-esquerdo do item (+4 px);
  limitado ao **retângulo do menu** (a superfície ocupa a tela inteira; o menu fica centralizado
  horizontalmente com `MARGIN`, embaixo ou em cima conforme `on_top`); inverte para cima se não couber
  (altura estimada `itens*36 + título`). Guardar o tamanho completo da superfície (hoje só a altura:
  `Resized`). Desenhar com `iced::widget::pin`/padding numa camada da pilha raiz. Comportamento do fundo
  (clicar fora fecha) inalterado. `User`, `Power`, `Category`, `Uninstall` não mudam de lugar.

## 5. Arrastar
- **Máquina de estados pura** (`src/menu/drag.rs`): `Idle → Armed{source, from} → Dragging{source, at,
  over}`; limiar de 6 px. Funções puras: `target_at(point, &[(Slot, Rectangle)]) -> Option<Slot>` e
  `apply(pinned: &[String], action, same_app) -> Vec<String>`. Ações: `Reorder{from_index, to_slot}`,
  `Pin{app, to_slot}`, `Cancel`. `Slot` = `Before(índice em self.pinned)` | `End`.
- **Origens:** `button.on_press_down_with_rectange` (arma; recebe o retângulo) **convivendo** com o
  `on_press` (Launch). O botão captura o clique esquerdo, então **não** se usa `mouse_area.on_press`.
  Quando o arrasto fica **ativo**, as origens são montadas sem `on_press` (`on_press_maybe(None)`),
  mantendo o `on_press_down`: soltar sobre a própria origem **não** lança o app. Sem passar do limiar
  nada muda: o `Launch` do botão é o clique (a máquina não emite `Click`).
- **Movimento e soltura:** subscription `event::listen_with` (CursorMoved e ButtonReleased(Left)),
  **ativa só fora de `Idle`**; vê eventos já capturados e usa coordenadas da janela. Sem `on_move`
  no `mouse_area` raiz e sem `on_enter/on_exit`. Esc, `Hide` e `Unfocused` cancelam o arrasto.
- **Alvos por hit-test puro:** ao armar, uma Task de bounds (a `Operation` do §4) consulta cada
  `pin:<i>` e a área de Fixados; `target_at` decide o alvo a cada movimento. Sem rolagem automática:
  só há alvo visível (Fixados ficam no topo do mesmo scrollable). A página Recomendados não tem alvo
  (soltar cancela).
- **Ids dos Fixados:** `self.pinned` guarda ids da dock e `find_app` os resolve de forma aproximada
  (wm_class, sufixo, caixa). Origem/alvo dos Fixados carregam o **índice em `self.pinned`**; "já fixado"
  usa a mesma regra de `find_app`. Soltar um app já fixado = reordenar. O item solto **ocupa o lugar do
  alvo**. Com Fixados recolhidos e >16 apps, soltar na área insere **depois do último visível**.
- **Origens válidas:** `pinned_tile`, tiles da grade de Todos (`all_grid`), linhas da lista, cards de
  Recomendados e ícones de Abertos **só se `app_for_window` existe**. **Não** são origem: a pasta de
  categoria (popup) e itens de busca.
- **Visual:** fantasma (ícone no cursor) numa camada raiz, alvo com realce, origem esmaecida.
  Persistência por `save_pinned`.

## Fora de escopo
Arrastar entre monitores, toque, reordenar "Todos", rolagem automática, animações.

## Testes
Puros: `px` (36→41, 20→23, 32→37), a máquina de arrasto (limiar, reorder para a esquerda e para a
direita, pin antes/no fim, cancel, alias e duplicata em `apply`, índice ausente), `target_at`, a posição
do popup (clamp direita/baixo/esquerda, inversão para cima). Visuais por screenshot: Fixados sem texto,
faixa de Abertos, popup ao lado do item. O arrasto em si **não é automatizável aqui** (sem injeção de
input): teste manual do usuário.
