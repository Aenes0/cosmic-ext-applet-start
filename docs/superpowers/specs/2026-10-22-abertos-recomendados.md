# Abertos numa linha com setas; Recomendados com os 4 apps mais recentes e selo "Novo" (spec)

Pedidos do usuário (2026-10-22):
1. "Abertos: quero que seja só uma linha porém com setas em ambas as direções quando mais apps estiverem
   abertos do que cabe em uma só linha."
2. "em recomendados, os quatro primeiros são os apps mais recentes com a tag novo se forem instalados ou
   atualizados nos últimos 15 dias."

Regras herdadas (ver `2026-10-22-menu-fixados-abertos.md`): sem commit; a estrutura da árvore de widgets
não muda condicionalmente sem necessidade (trocar estrutura perde estado); ícones de app em `px(36)`;
símbolos de interface (setas) **sem** `ICON_SCALE`; tooltip com atraso de 500 ms.

---

## 1. Abertos: uma linha só, setas ‹ ›

### 1.1 Comportamento
- A faixa cinza (`open_strip`) passa a ter **uma única linha** de ícones. Título "Abertos" inalterado.
- Se todas as janelas cabem: linha sem setas (como hoje, só que nunca quebra).
- Se `total > capacidade`: seta **‹** à esquerda e **›** à direita da linha, botões com
  `go-previous-symbolic` / `go-next-symbolic` (16 px, `flat_class(p, false)`). Seta sem destino (no
  início/fim) fica **desabilitada** (`on_press_maybe(None)`), não some — a linha não "pula" de largura.
- **Clique pagina uma página inteira** (avança `capacidade` itens), com a última página **encostada no
  fim** (`offset = min(offset + cap, total - cap)`), e a primeira no início (`offset.saturating_sub(cap)`).
  Justificativa: o usuário quer *ver* o que está escondido; com 9–10 por página, um item por clique exige
  muitos cliques; página encostada no fim nunca mostra buracos e mantém a linha cheia.
- **Sem roda do mouse.** A faixa está dentro do `scrollable` do `home`; um `mouse_area.on_scroll` capturaria
  a roda e travaria a rolagem vertical da página sobre a faixa. Setas bastam.
- Preservados sem mudança: `open_icon` (tooltip `título — app`, id `scoped("open", id)`, origem de arrasto
  `drag::Source::App`, clique ativa, clique direito → `MenuFor(Target::Window…)`, realce da ativa).

### 1.2 Capacidade (por largura, constante — o menu tem largura fixa)
Largura útil da linha = `WIDTH - 2*SIDE - 2*12` (padding da faixa) = 874 − 92 − 24 = **758 px**.
Célula de ícone com **largura fixa** `OPEN_CELL = px(36) + 2*12 = 41 + 24 = 65 px` (deixa de ser
`Length::Fill`); espaçamento `OPEN_GAP = 8`; seta `OPEN_ARROW = 28` px de largura (+ `OPEN_GAP` ao lado).
- sem setas: `n*65 + (n-1)*8 ≤ 758` ⇒ **10**;
- com setas: largura útil `758 - 2*(28+8) = 686` ⇒ **9**.
`capacity(total)` devolve 10 se `total ≤ 10`, senão 9. Os números saem de funções puras com as constantes
como parâmetros (teste fixa 10/9 para os valores reais). Ícones alinhados à esquerda; sobra à direita.

### 1.3 Estado e funções puras — `src/menu/open_strip.rs` (novo)
```rust
pub const CELL: f32; pub const GAP: f32; pub const ARROW: f32; pub const INNER: f32; // 65, 8, 28, 758
pub fn fit(width: f32, cell: f32, gap: f32) -> usize;          // max n: n*cell+(n-1)*gap ≤ width (≥1)
pub fn capacity(total: usize) -> usize;                         // 10 se cabe tudo, senão 9 (via fit)
pub fn visible_range(total: usize, cap: usize, offset: usize) -> std::ops::Range<usize>; // clampado
pub fn clamp(total: usize, cap: usize, offset: usize) -> usize; // min(offset, total.saturating_sub(cap))
pub fn can_prev(offset: usize) -> bool;                         // offset > 0
pub fn can_next(total: usize, cap: usize, offset: usize) -> bool; // offset + cap < total
pub fn next(total: usize, cap: usize, offset: usize) -> usize;  // min(offset+cap, total-cap) (clamp)
pub fn prev(cap: usize, offset: usize) -> usize;                // offset.saturating_sub(cap)
pub fn reveal(total: usize, cap: usize, offset: usize, index: usize) -> usize;
    // se index fora da janela: menor deslocamento que o mostra (index < offset ⇒ index;
    // index ≥ offset+cap ⇒ index+1-cap); depois clamp
```
Testes (TDD): `fit` 758→10 e 686→9; `capacity(0..=10)=10`, `capacity(11)=9`; `visible_range` com
total ≤ cap = `0..total`; offset além do fim é clampado (janelas fecharam: total 12→9 com offset 3 ⇒ 0);
`next` encosta no fim (total 20, cap 9: 0→9→11→11); `prev` 11→2→0; `can_prev/can_next` nas bordas;
`reveal` à esquerda, à direita, já visível (não mexe), total ≤ cap ⇒ 0.

### 1.4 Hook em `menu.rs` (mínimo)
- Estado: `open_offset: usize` e `open_active: Option<String>` (id da última janela ativa vista).
- `Message::OpenPage(bool /* true = próxima */)` → `open_offset = next|prev(...)`.
- `Message::Windows(windows)`: atribui; `cap = capacity(len)`; se o id da ativa mudou e existe, `offset =
  reveal(len, cap, offset, idx_ativa)`; senão `offset = clamp(len, cap, offset)`; grava `open_active`.
- `show()`: ao abrir o menu, `open_offset = reveal(len, cap, 0, idx_ativa)` (ou 0 sem ativa).
- `open_strip`: `row` única = [seta ‹]? + ícones de `visible_range` (cada `open_icon` envolto num
  container `Length::Fixed(CELL)`; o `container` interno de `open_icon` que hoje é `Fill` passa a
  `Fixed(CELL)`) + [seta ›]?. Setas presentes **somente** quando `total > capacity(total)` (pedido do
  usuário); quando presentes, desabilitam em vez de sumir.

---

## 2. Recomendados: os 4 apps mais recentes primeiro, selo "Novo" até 15 dias

### 2.1 Comportamento
- As **4 primeiras** posições de `picks()` são os 4 apps com `added` mais recente (instalados/atualizados),
  **independente de idade** — lê-se o pedido como "os 4 primeiros são os apps mais recentes"; a etiqueta
  "Novo" é o qualificador: só quem tem `now - added ≤ 15 dias` (1.296.000 s, **inclusive**) recebe o selo.
  Os demais dos 4 aparecem sem selo.
- O resto da lista continua como hoje — apps abertos daqui (`launched`) e arquivos (`recents`) por
  recência — **sem repetir** os 4. O bloco atual "Adicionado recentemente" (filtro de 14 dias) **sai**,
  substituído por este.
- Subtítulo dos 4: `texts().installed` + " · " + `ago(added)` (ex.: "Instalado · Há 3 dias"); o selo fica
  à direita do texto no card.
- Home mostra `CARD_COLUMNS*CARD_ROWS = 6` ⇒ 4 apps novos + 2 mais recentes do resto; página
  Recomendados (até 30) idem, com os 4 primeiro.

### 2.2 Fidelidade de `added` (limitações e regra mínima)
- **pacman**: extrai o `.desktop` de novo em toda instalação/atualização ⇒ *birth time* (`created()`) =
  momento da instalação. O `mtime` vem do pacote (data do **build**, pode ser dias antes) ⇒ `modified()`
  é pior; por isso **não** usar `max(created, modified)` como fonte primária.
- **Flatpak**: `exports/share/applications/*.desktop` é symlink; `metadata` segue para o arquivo do
  deploy, recriado a cada atualização ⇒ birth time correto.
- **AppImage/arquivo solto em `~/.local/share/applications`**: birth = quando o integrador/usuário criou.
- **Falso positivo conhecido**: restaurar backup/copiar a pasta (`cp -a`, rsync, snapshot restaurado)
  gera birth time novo ⇒ app aparece como "Novo" por 15 dias. Aceito; documentar.
- **Sistema de arquivos sem birth time**: cai no `modified()` (data de build no pacman) — subestima.
- **Instalação inicial do SO**: todos os apps nascem juntos; se o SO tem < 15 dias, todos seriam "Novo".
  Regra: o selo exige também `added > baseline + 3600`, onde `baseline` = menor `added` conhecido (> 0)
  da lista (mesma heurística do `picks()` atual). A ordem dos 4 não usa esse filtro.
- **Regra mínima em `load_apps`**: `added = created` se `0 < created ≤ now + 60`; senão `modified` se
  `0 < modified ≤ now + 60`; senão `0` (desconhecido). Futuro (relógio torto, arquivo de outra máquina)
  não vira "mais recente para sempre". Implementar como função pura `pick_added(created: Option<i64>,
  modified: Option<i64>, now: i64) -> i64` em `recent.rs`, chamada por `load_apps`.

### 2.3 Função pura — `src/menu/recent.rs` (novo)
```rust
pub const NEW_FOR: i64 = 15 * 86_400; // 1_296_000
pub const SKEW: i64 = 60;
pub fn pick_added(created: Option<i64>, modified: Option<i64>, now: i64) -> i64;
/// Índices em `added` dos `n` mais recentes (desc.), com o selo "novo".
/// Ignora added ≤ 0 e added > now + SKEW. Empate: menor índice primeiro (ordem estável =
/// ordem alfabética de `apps`, já ordenada por nome). novo ⇔ now - a ≤ NEW_FOR && a > baseline + 3600,
/// baseline = min dos added válidos.
pub fn newest_apps(added: &[i64], now: i64, n: usize) -> Vec<(usize, bool)>;
```
Recebe `&[i64]` (não `&[AppEntry]`) para testar sem montar `AppEntry`; o chamador faz
`apps.iter().map(|a| a.added).collect()`. `now` injetado (relógio testável).

Testes (TDD): borda exata `now - added == NEW_FOR` ⇒ novo; `NEW_FOR + 1` ⇒ não novo mas ainda entre os 4;
empate ⇒ ordem por índice; menos de 4 apps válidos ⇒ devolve só os válidos; `added = 0` ignorado; futuro
`now + 61` ignorado e `now + 60` aceito; baseline: lote de instalação do SO (todos iguais) ⇒ nenhum novo;
`n = 0` ⇒ vazio. `pick_added`: created válido vence; created futuro ⇒ modified; ambos `None` ⇒ 0.

### 2.4 Hook em `menu.rs` / `texts.rs` (mínimo)
- `texts.rs`: `new_badge: "Novo"` / `"New"`; `installed: "Instalado"` / `"Installed"`. Remover
  `recently_added` **só** se não restar uso (grep).
- `enum Pick`: `App(&AppEntry, String, bool /* novo */)`.
- `picks()`: `top = recent::newest_apps(&added, now(), 4)`; montar `head` com esses (subtítulo §2.1,
  `seen.insert`); depois `launched` + `recents` filtrando `seen`, ordenados por `-when` como hoje;
  devolver `head` seguido do resto (o `sort` final passa a ser só do resto).
- `card(...)`: novo parâmetro `badge: Option<&'static str>`; quando `Some`, empurra à direita do texto
  uma pílula: `container(text::caption(badge).size(11))` com `padding([1, 8])`, fundo `p.accent`, raio
  999 (pílula), texto em cor de contraste (branco ou `p.bg` — usar o que já se usa sobre `p.accent` no
  código; grep). `pick_card` passa `new.then_some(texts().new_badge)`; `Pick::File` passa `None`.
- `load_apps`: `added` via `recent::pick_added(created, modified, now())`.

---

## 3. Tarefas (operários Sonnet low; arquivos disjuntos; TDD; não commitar)

**Onda 0 (sessão principal, antes do dispatch):** adicionar em `src/menu.rs`, junto aos outros `mod`
(linhas ~77–80), `mod open_strip;` e `mod recent;` — evita corrida de dois operários na mesma linha.

**Onda 1 (paralelo):**
- **T1 — `src/menu/open_strip.rs`** (só este arquivo): constantes + funções do §1.3 + `#[cfg(test)]`
  com os casos do §1.3. Verificação: `flatpak-spawn --host cargo test open_strip` (no diretório do
  projeto) → todos verdes. Avisos de `dead_code` do módulo são esperados até T3 e **não** devem ser
  calados com `#[allow(dead_code)]` (somem quando o hook usar as funções).
- **T2 — `src/menu/recent.rs`** (só este arquivo): §2.3 + testes. Verificação: `cargo test recent` verde.

**Onda 2 (sequencial, mesmo arquivo `menu.rs`, regiões disjuntas):**
- **T3 — hook Abertos** (`menu.rs`: struct `StartMenu` + `new`, `enum Message` (`OpenPage`), `update`
  (`OpenPage`, `Windows`), `show()`, `open_icon` (largura fixa), `open_strip`). Verificação:
  `cargo build` + `cargo test` verdes; manual: abrir 12+ janelas → linha única com ‹ ›; ‹ desabilitada no
  início; › leva ao fim encostado; fechar janelas não deixa buraco; ≤ 10 janelas sem setas.
- **T4 — hook Recomendados** (`menu.rs`: `enum Pick`, `picks()`, `card()`, `pick_card()`, `load_apps`
  (`added`); `texts.rs`). Verificação: `cargo build` + `cargo test` verdes; manual: 4 primeiros cards são
  apps, selo "Novo" só nos ≤ 15 dias; resto sem duplicar.

Licença de corrigir o plano (todas as tarefas): se o código do brief não compilar, contradisser as
assinaturas reais ou falhar nos testes do próprio brief, o plano pode estar errado — corrija para
satisfazer o teste e relate a divergência; nunca altere o teste para acomodar implementação defeituosa.

## 4. Fora de escopo
Roda do mouse na faixa; animação de paginação; arrastar entre páginas da faixa; mudar `app_tile`, a grade
de Fixados ou a página Todos; persistir `open_offset` entre aberturas.
