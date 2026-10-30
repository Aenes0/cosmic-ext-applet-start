# Revisão crítica do plano "leveza" do `--menu` antes da produção (2026-10-22)

> Escopo: só revisão e leitura. Nada do repositório nem dos experimentos foi alterado; este arquivo é a única
> escrita. Documentos lidos: `leveza-pilha.md` (principal), `leveza-piso.md`, `leveza-referencias.md`,
> `perf-runtime.md` e `perf-build.md`. Também li o diff real de `target/exp-leve-cosmic` contra o checkout
> `~/.cargo/git/checkouts/libcosmic-41009aea1d72760b/d4d71fd`, o protótipo `target/exp-leve2`, a sonda
> `leve_probe.rs`, `target/exp-leve-scripts/shots.sh` e o código atual (`src/main.rs`, `Cargo.toml`,
> `src/menu.rs`, `src/menu/{application,load,launch,style}.rs`).
>
> Rótulos: **[verificado]** foi lido no código, com arquivo e linha. **[medido]** vem dos specs.
> **[hipótese]** não foi verificado.

## Veredito

**É seguro começar, com condições.** A direção está certa: tiny-skia mais P1 e P2, ícones pré-rasterizados,
pageout e o backend trocado por último.

Há quatro pontos que impedem produção do jeito que o plano está escrito:

1. **O `ICED_BACKEND` vaza para os apps lançados (F1).**
2. **Tirar a feature `wgpu` impede o rollback por variável (F2).**
3. **A bateria de pixel nunca exercitou o P1 (F3).**
4. **As metas são fisicamente inalcançáveis (F4).**

Também é preciso tirar o fork de `target/` (F5).

Precisam do OK do usuário: (a) adotar o fork local e decidir onde ele mora; (b) a mudança visual (sombra,
anti-aliasing do texto, bordas de ícones); (c) as metas revisadas; (d) a troca do backend.

## Veredito por ponto

| # | Ponto | Veredito |
|---|---|---|
| 1 | P1 e P2 | **Corretos por construção** [verificado]. A cobertura de teste é **insuficiente** (F3). P2 precisa de endurecimento (F6). |
| 2 | Fork por `[patch]` | **Viável.** Está **frágil** onde está: dentro de `target/`, com caminhos absolutos e o libcosmic sem `rev` (F5). Não afeta os outros applets [verificado]. Afeta o **botão do painel**, que é o mesmo binário (F8). |
| 3 | Ícones | **Viável.** O caminho exato do renderizador está descrito em F9: o PNG usa alvo de 44 px, não 41, e o SVG usa o tamanho ajustado, sem centralizar num quadrado. Precisa ficar restrito ao modo CPU (F17). |
| 4 | `MADV_PAGEOUT` | **Seguro** se ajustado: sair da thread da UI, atraso maior, conferência de "visível" e só no modo CPU (F10). |
| 5 | Sombra | **Emular com anéis de borda**, não com retângulos cheios nem com o `if false` do engine (F11). Exige OK visual. |
| 6 | Superfície de tela cheia | **Confirmado:** não há alternativa simples (F13). Adiar. |
| 7 | Transparência e blur | **OK** [verificado e medido] (F14). |
| 8 | Seleção do backend | **Código em `main.rs`**, não no `Exec`: não existe `.desktop` para o `--menu`. O rollback deve ser uma chave de config. Os filhos precisam restaurar o `ICED_BACKEND` (F1, F2). |
| 9 | Metas | **Irreais** como estão (F4). Ver a tabela revisada. |
| 10 | Ordem e divisão | Ajustada na seção "Ordem de produção". |

## Findings

### F1 — HIGH: `ICED_BACKEND=tiny-skia` vazaria para todo app aberto pelo menu

**Onde:** `launch.rs:102` (`spawn_desktop_exec(exec, [workers()], …)`) e `launch.rs:18` (`spawn`), que é
usado em `application.rs:346`, `:351` e `:429`. Os dois herdam o ambiente do processo [verificado].

**Problema:** com `set_var("ICED_BACKEND","tiny-skia")` no ramo `--menu`, o cosmic-files, o cosmic-term e
os outros abririam em CPU. Hoje isso não acontece porque o `--menu` não mexe na variável. O `spawn_clean`
já a restaura, mas só quando `BACKEND` foi inicializado, o que hoje só ocorre no ramo do botão.

**Correção:**
1. No ramo `--menu`, fazer `BACKEND.get_or_init(|| var_os("ICED_BACKEND"))` **antes** do `set_var`.
2. Criar `restore_backend(&mut Command)` e usar em `spawn()`.
3. Em `spawn_desktop_exec`, passar também `("ICED_BACKEND", original ou "")`. Para o iced, `""` equivale a
   ausente, porque `fallback.rs:286-299` filtra candidatos vazios [verificado].

**Teste puro:** `child_backend_env(None) == ""` e `child_backend_env(Some(v)) == v`.

### F2 — HIGH: "sem a feature `wgpu`" e "rollback por uma variável" são incompatíveis

Se a feature sair, voltar ao wgpu exige rebuild. O ganho de tirá-la é pequeno [medido]:
- RssFile −0,9 MB;
- binário de 35,8 para 28,3 MB;
- tiny-skia com a feature: 29,1–30,3 MB, contra 28,9–29,8 MB sem ela.

**Correção:**
- **Manter a feature** na primeira entrega e removê-la numa tarefa separada, depois de 1 a 2 semanas sem
  regressão.
- Rollback por uma **chave cosmic-config** `menu_backend = "wgpu"`, lida em `main.rs` antes de
  `cosmic::app::run`. A config é melhor que uma variável de ambiente porque o `--menu` nasce do botão
  (`start_button.rs:48`, via `spawn_clean`), de `tools.rs:48` e do atalho `--tools`. Não há `Exec` a editar,
  e o ambiente do painel é difícil de mudar.
- Um `ICED_BACKEND` explícito do usuário vence a chave.

**Teste puro:** `menu_backend(env, config) -> Option<&str>`, com a tabela de decisão completa.

### F3 — HIGH: nenhum teste de pixel exercitou o P1

**Problema:**
- O `shots.sh` captura **só o primeiro quadro** depois de abrir. Com idade 0, o dano é a tela inteira, então
  o P1 nunca entra (`damage.len() > 2` é falso). O P2 entra, mas num quadro completo.
- A sonda `leve_frames` mede tempo, não compara pixels. Além disso, ela **reimplementa** o P1
  (`leve_probe.rs:170`) em vez de chamar o código do fork.
- Rolagem, popup, arrasto, prévia e resize ficaram sem verificação de pixel.

**Correção:**
1. No fork, expor o P1 como função pública, por exemplo
   `iced_tiny_skia::window::compositor::coalesce_damage(Vec<Rectangle>) -> Vec<Rectangle>`, e usá-la em
   `present` e no teste.
2. Criar um **teste oráculo headless** (`#[ignore]`, rodado explicitamente):
   - Para cada quadro dos cenários, comparar o buffer desenhado **só no dano**, com P1 e P2, idade 2 e 2
     buffers alternados, contra o **redesenho completo com máscara nova**. O resultado tem de ser
     **byte-idêntico**.
   - Cenários: rolagem de Lista e Grade (30 para baixo, 30 para cima), hover entre quadros, popup de
     contexto abrindo e fechando, troca Início → Todos → Início, busca, tools, prévia com miniatura
     sintética, fantasma de arrasto (estado posto por mensagens `Drag*`), mudança de métricas (resize) e
     troca do tamanho da superfície (máscara recriada).
   - **[hipótese]** A igualdade exata é esperada, porque a região de dano é limpa com `BlendMode::Source` e
     tudo é redesenhado. Se o stock também divergir do oráculo, comparar o patch contra o stock.

### F4 — HIGH: as metas propostas não são alcançáveis

| Meta proposta | Medido | Por quê |
|---|---|---|
| RSS escondido ≤ 12 MB (alvo 8) | 23,1–23,5 com pageout | RssFile de 20,9 MB, dos quais 17,5 MB são o `.text`/`.rodata` do binário, compartilhado com os botões. **[hipótese]** O PAGEOUT pula folios compartilhados. |
| Visível ≤ 25 MB, shm ≤ 4 | 74–77,5 MB | Só o shm de tela cheia já tem 37,5 MB. |
| Quadro ≤ 6,9 ms | mediana de 8,3–10,3 | Exige um patch de dano por bounds **[hipótese]**. |
| 1º quadro p95 ≤ 50 ms | toggle → `view` de 69,5 ms | Show → configure sozinho leva 65–85 ms. |
| Threads ≤ 3 | 5 | — |

Ver as metas revisadas abaixo.

### F5 — HIGH: o fork vive em `target/`

**Problema:**
- `cargo clean` apaga o fork e quebra o build.
- O `Cargo.toml` do protótipo usa caminhos absolutos dentro de `target/`.
- O `libcosmic` não tem `rev` no `Cargo.toml`, e o lock fixa d4d71fd. Com os 18 crates trocados por caminho,
  um `cargo update` ainda move as dependências transitivas (cosmic-text, sctk e outras) para longe do que
  o d4d71fd espera **[hipótese de quebra]**.
- O `iced` é **submódulo** do libcosmic (`.gitmodules` [verificado]).

**Correção:**
- No repositório (quem commita é o usuário):
  - `patches/libcosmic/0001-tiny-skia-damage-bbox.patch`;
  - `0002-tiny-skia-clip-mask-last-rect.patch`;
  - `scripts/libcosmic-patched.sh`, idempotente.
- O script gera `~/Projetos/libcosmic-patched` a partir do checkout do cargo em d4d71fd:
  - confere `git rev-parse HEAD = d4d71fd53e5ed6bd3a430089114dffa2da3cd498` e o commit do submódulo `iced`;
  - se o checkout tiver sido removido pelo GC do cargo, usa `git clone --recurse-submodules` +
    `checkout` como fallback;
  - aplica com `git apply --check` e depois `git apply`.
- No `Cargo.toml`:
  - adicionar `rev = "d4d71fd53e5e…"` na dependência `libcosmic`;
  - adicionar o `[patch."https://github.com/pop-os/libcosmic"]` com os 18 caminhos sob
    `~/Projetos/libcosmic-patched`;
  - manter os `[patch]` de sctk e cosmic-protocols.

**Alternativa menor, com timebox de 30 min [hipótese]:** fazer o `[patch]` só do `iced_tiny_skia`.
- O crate precisa ser "desworkspaceado": `version` e `features` explícitos, e `iced_graphics`/`iced_debug`
  apontando para o **mesmo** `git = "https://github.com/pop-os/libcosmic"`, com a mesma spec/rev da raiz.
  Uma spec diferente gera outra *source id*, ou seja, um segundo `iced_graphics` e tipos incompatíveis.
- Aceite: `cargo tree -d` sem crates `iced_*` duplicados e `cargo tree -i iced_tiny_skia` apontando para o
  caminho local.
- Se falhar, ficam os 18 crates.

**Verificado:** `taskbar` e `quick-settings` têm `Cargo.toml` próprio, sem `[patch]` do libcosmic, e não
existe `~/.cargo/config.toml`. O fork não os afeta.

### F6 — MEDIUM: endurecer o P2 (`engine.rs` `clear_last`)

**Correção confirmada por leitura [verificado]:**
- Toda escrita na máscara passa por `adjust_clip_mask`: `engine.rs:883-910`, chamado de `lib.rs:114/142/155`
  e `engine.rs:370/409/445`.
- Toda `Mask` nasce zerada: `compositor.rs:72`, `:99` e `screenshot:240`.
- Logo, a invariante "máscara = último retângulo" se mantém. Uma entrada velha (endereço reaproveitado depois
  de um resize) só zera o que já era zero e **não deixa sujeira**.
- Com várias saídas, há uma máscara por superfície.
- Em outra thread, o `thread_local` dá `None`, o que leva a `clear()` completo: seguro.

**Ajustes de produção:**
1. Tirar o `std::env::var_os` do caminho quente, no P2 e no P1. O `adjust_clip_mask` roda por camada × dano
   × grupo e por primitiva, e cada leitura faz lock e alocação.
2. Chavear por `(ptr, w, h)`.
3. Se o retângulo novo for igual ao anterior, retornar cedo, sem limpar nem preencher. O `draw_primitive`
   reaplica o mesmo retângulo a cada primitiva.
4. Guarda defensiva `px0.min(px1)` antes do fatiamento. Hoje não é bug: `Rectangle::intersection` só devolve
   largura > 0 (`core/rectangle.rs:234` [verificado]). Mas com `panic = "abort"` um pânico derruba o menu
   **e os botões do painel**.

### F7 — MEDIUM: P1 correto; a oportunidade está no dano do Wayland

**Correto [verificado]:**
- A união só cresce.
- A região é limpa com `Source` e todas as camadas que a cruzam são redesenhadas (`lib.rs:79-104`). Não há
  blend duplo nas translúcidas, e a máscara não é afetada.
- `present()` → `present_with_damage(buffer inteiro)` (softbuffer `lib.rs:448`, `wayland/mod.rs:205-222`):
  o `wl_surface.damage_buffer` já é a tela inteira, e o P1 não muda isso.

**Opcional, P3 [hipótese]:** chamar `present_with_damage(bbox físico)` pouparia ao cosmic-comp o upload de
cerca de 19 MB por quadro. Medir a CPU do compositor. Não entra na primeira onda.

### F8 — MEDIUM: o fork entra também nos 3 botões do painel

É o mesmo binário, e o botão já usa tiny-skia (`main.rs:56-62`). **Correção:** incluir o painel (o botão)
na bateria de captura antes e depois, e no período de observação.

### F9 — HIGH: ícones pelo caminho **exato** do renderizador

O protótipo centralizou e usou CatmullRom. O caminho real [verificado] é este.

**SVG** (`vector.rs:44-60`, `:138-178`):
- rasteriza em `(bounds.w as u32, bounds.h as u32)` **lógicos**;
- usa `tree.size().to_int_size()`, depois `scale_to_width` se w > h e `scale_to_height` caso contrário;
- aplica `Transform::from_scale(alvo/árvore)` **sem translação**;
- usa o `usvg::Options` com o **fontdb do cosmic-text** (`vector.rs:96-110`);
- desenha em `(x as i32, y as i32)` com Bicubic.

O widget `svg` com `ContentFit::Contain` já encaixa e centraliza SVGs não quadrados. Por isso, pré-rasterizar
no **tamanho encaixado** (não quadrado); o widget `image` faz o mesmo encaixe.

**PNG** (`raster.rs:59-66`, `graphics/image.rs:188-212`):
- `downsample_target` quantiza para múltiplo de 4 (**41 → 44**) e só reamostra se a origem for ≥ 1,25 × e
  pequena;
- depois desenha 44 → 41 com Bilinear.

Portanto, pré-rasterizar o PNG com `cosmic::iced::advanced::graphics::image::{downsample_target,
downsample_premultiplied}` (públicas) **no mesmo alvo**, e não em 41. Se `downsample_target` der `None`,
manter o handle original.

**Outros cuidados:**
- **Símbolos:** `handle.symbolic` é recolorido conforme o tema, e a chave do cache inclui a cor. Não
  pré-rasterizar esses.
- **HiDPI:** o stock rasteriza no tamanho **lógico** e escala pela transformação (`vector.rs:46`, "XX Do not
  apply transform scaling"). Pré-rasterizar em 41 e 28 lógicos, sem multiplicar pela escala. Fica igual nos
  dois caminhos.
- **Tamanhos:** só `px(36)` = 41 e `px(24)` = 28 (`ICON_SCALE` = 1,15, `style.rs:4`). Os de 48, `icon_max`,
  16 e 64 continuam SVG.
- **`resvg`:** `"=0.45.1"`, a mesma versão do lock. As features unificam.
- **Tempo:** cerca de +114 ms em `load_apps`, fora da UI e só quando os apps mudam. A primeira abertura a
  frio leva cerca de 0,6 s até a lista **[estimado]**.
- **Memória:** +0,7 MB [medido].

**Aceite:**
- Teste in-process: ícone SVG ou PNG desenhado pelo widget contra o pré-rasterizado, diferença máxima ≤ 2.
- Captura: 0 px acima de 8 em Início, Todos (lista e grade) e busca.

### F10 — MEDIUM: implementação do pageout (`src/menu/trim.rs`)

O protótipo (`exp-leve2/src/menu.rs`) acerta no essencial. Corrigir:
1. Trocar o `21` por `libc::MADV_PAGEOUT` (existe no libc 0.2.186 [verificado]). Nunca usar
   `DONTNEED`/`FREE`.
2. Tirar da thread da UI: o protótipo roda dentro do `update`. Comprimir cerca de 6 MB leva **[estimado]**
   6–12 ms. Usar `std::thread`/`spawn_blocking`.
3. Usar um timer **próprio, de cerca de 10 s**, não os 500 ms do trim, com geração do hide. Assim uma
   reabertura rápida não paga 1,3–1,9 ms/MiB de repaginação.
4. Antes de cada faixa, abortar se `WINDOWS_SHOWN` for verdadeiro.
5. Filtro de mapeamentos:
   - incluir só `rw-p` com caminho vazio ou `[heap]`;
   - excluir `rw-s` (memfd/shm), `[stack]`, `[vvar]`, `[vdso]` e arquivos.
6. Parsing sem `unwrap` nem indexação (`panic = abort`), com `checked_sub`.
7. Tolerar `EINVAL`/`ENOMEM` por faixa. Sem swap, vira no-op.
8. Um único `unsafe`, com SAFETY documentado: é só um conselho ao kernel, o conteúdo se preserva e outras
   threads que escrevem apenas refazem o fault.
9. **Ativar só no modo CPU:** no wgpu, há mapeamentos anônimos do driver NVIDIA.

### F11 — MEDIUM: a sombra

- Ligar o `if false` (`engine.rs:80`) **não** serve: ele calcula SDF + blur por pixel em todo o retângulo da
  sombra (cerca de 0,9 Mpx) a cada redesenho, e há "glitches" não diagnosticados.
- Proposta: **N = 6 a 8 quads só de borda** (anéis de largura `b/N`, `b` = 16, raio 8 + offset), com alfa por
  anel dado por `0,45·(1 − smoothstep(−b, b, d))`.
  - **[hipótese]** É a fórmula do shader de quad do iced wgpu; conferir no `.wgsl` do d4d71fd.
  - Só a área do anel é rasterizada (cerca de 0,1 Mpx). Não custa nada no resize, e a cor vem da palette
    clara ou escura.
- Desenhar **só se o backend for CPU**, por uma flag estática ligada em `main.rs`, para não duplicar a sombra
  no rollback para wgpu.
- Aceite contra a captura wgpu: hoje são 19.723 px diferentes fora do cartão; meta de p99 ≤ 12/255 na faixa
  da sombra.
- Exige OK visual.

### F12 — MEDIUM: CPU da prévia no tiny-skia não foi medida

Cada atualização da prévia (1 s) cria um handle novo, que passa por reamostragem Lanczos e redesenho. No
wgpu, a prévia aberta custa 2,75% [medido].

Entra no roteiro manual: `pidstat -p PID 1` com a prévia aberta por 30 s. Aceite ≤ 3% de um núcleo.

### F13 — LOW: superfície de tela cheia (confirmado, adiar)

Confirmado [verificado]:
- `menu.rs:431-447` ancora nos 4 lados com teclado `Exclusive`.
- `input_zone`/`set_input_region` só restringe a entrada: o clique no papel de parede passaria direto e não
  fecharia o menu.
- A região opaca não reduz o buffer, e o buffer tem sempre o tamanho configurado (`compositor.rs:85-101`).

Um backdrop de 1×1 exige uma de duas coisas:
- mudar o fork (todo `Window` cria compositor e buffer);
- ou uma segunda layer surface feita com sctk cru, `single-pixel-buffer` e viewporter, com o conteúdo num
  surface do tamanho do menu. Há risco de ordem de empilhamento e foco **[hipótese]**.

O ganho é de cerca de 30 MB **só enquanto visível**. Fora desta onda.

### F14 — LOW: transparência e blur estão OK

- O softbuffer do pop usa `Argb8888` (`wayland/buffer.rs:101`) [verificado].
- Capturas: 0 px de diferença fora dos ícones entre base tiny-skia e leve. Entre wgpu e tiny-skia, só a
  sombra e o anti-aliasing do texto (12 mil px dentro do cartão) [medido].
- `set_auto_blur(Default)` é protocolo do compositor e independe do renderizador.

### F15 — LOW: o `thread_local` do P2 acumula entradas

Fica uma entrada por endereço de máscara (o `screenshot()` cria uma por chamada). É desprezível. Some com a
chave `(ptr, w, h)` e o retorno cedo.

### F16 — LOW: o `exp-leve2` é um snapshot antigo do `src`

Ele não tem `icon_cache.rs`, `pointer_stats`, `id_lower`/`app_for_id` nem os testes de `find_app`.
**Não copiar arquivos do protótipo:** reimplementar sobre o código atual.

### F17 — MEDIUM: ícones e pageout precisam ser condicionais ao modo CPU

Se os ícones RGBA entrarem enquanto o menu ainda está em wgpu, o visual do wgpu muda, porque ele rasteriza
SVG em tamanho físico no atlas. Isso também suja o rollback.

**Correção:** `AppEntry::icon_at(slot)` devolve o RGBA só com a flag de CPU ligada; o pageout segue a mesma
flag.

**Ponto aberto (sem dono na divisão abaixo):** o F11 também põe a flag estática em `main.rs`, mas o F17 não
diz onde ela mora. A `style.rs` da tarefa D só chega na onda 3, e as tarefas B e C (onda 1) já leem a flag.
O dono da flag na onda 1 ainda precisa ser decidido. Uma opção é a tarefa E-prep, que já edita `main.rs`.

**Ainda aberto:** `apps_signature` (`load.rs:134`) não inclui o tema de ícones. Isso é pré-existente e não
foi introduzido por este plano. Opcional: incluir o nome do tema.

## Metas revisadas (no lugar das de `leveza-referencias.md` §4)

| Métrica | Escondido (10 s depois do hide) | Visível parado |
|---|---|---|
| Pss_Anon / RssAnon | ≤ 3 MB com pageout (medido 2,2–2,5), ≤ 9 MB sem | ≤ 25 MB fora do shm |
| RSS | ≤ 25 MB com pageout, ≤ 31 MB sem | ≤ 80 MB (37,5 de shm de tela cheia) |
| PSS (binário de produção, compartilhado com os botões) | ≤ 12 MB **[estimado]** | — |
| Wakeups / CPU / VRAM | **0 / 0 / 0** | ≤ 35/s parado, VRAM 0 |
| Threads | ≤ 5 | ≤ 9 |
| Rolagem (sonda, tela cheia) | — | mediana ≤ 10,5 ms, p90 ≤ 14 ms (6,9 ms fica como aspiração, com o P3/diff por bounds) |
| Abertura | — | toggle → `view` com mediana ≤ 80 ms; 1º quadro ≤ 110 ms **[estimado]** |

## Ordem de produção (ajustada) e divisão para operários Sonnet

**Fase 0, OK do usuário:**
1. Fork local em `~/Projetos/libcosmic-patched`, gerado pelo script, e pin de `rev`.
2. Metas revisadas.
3. Sombra: aceitar sem ela, ou emular.
4. A troca do backend fica para a última onda.

**Onda 1** (em paralelo, arquivos disjuntos; o `Cargo.toml` só na tarefa A):

| Tarefa | Arquivos | Verificação |
|---|---|---|
| **A — fork** | `patches/libcosmic/*.patch`, `scripts/libcosmic-patched.sh` e `Cargo.toml` (`rev` + `[patch]`, mais a linha `resvg = "=0.45.1"` para a B). Patches de produção sem variáveis de ambiente, com as guardas do F6 e `coalesce_damage` pública. | `cargo build --release`; 377 testes verdes; `cargo tree -d` sem `iced_*` duplicado. |
| **B — ícones** | `src/menu/icon_raster.rs` (novo, puro), `load.rs`, struct em `menu.rs`, `tiles.rs`, `pages.rs`, `search.rs` e `drag_glue.rs`. Condicional ao modo CPU (F17). Depende da linha de `resvg` da A. | Testes puros e o teste de equivalência do F9. |
| **C — pageout** | `src/menu/trim.rs` (novo) e o braço `Trim` em `application.rs`. O `mod trim;` em `menu.rs` é inserido **pelo integrador depois da B**, que também mexe em `menu.rs`. | Testes de parsing com fixture de `/proc/self/maps`. |
| **E-prep** | `main.rs` (`menu_backend`, `BACKEND` no ramo `--menu`) e `launch.rs` (`restore_backend`, env no `spawn_desktop_exec`), **com o padrão ainda wgpu**: nenhuma mudança visível. | Testes de decisão e de restauração do ambiente. |

**Onda 2:** o teste oráculo de pixel (F3), em `src/menu/render_oracle.rs` (`#[cfg(test)] #[ignore]`).
Depende da A e da B.

**Onda 3:** **D — sombra**, depois do OK. Mexe em `style.rs`, na região da `view` do cartão em
`application.rs` (sequencial à C, mesmo arquivo) e na flag de CPU.

**Onda 4:** **E-flip**.
- Padrão tiny-skia no `--menu`, numa linha da função `menu_backend`.
- Bateria ao vivo (shots wgpu contra tiny-skia): Início, `--tools`, tema claro e escuro (alternado pela
  cosmic-config e restaurado), o botão do painel, e `live.sh`/`lat.sh`.
- Roteiro manual do usuário, abaixo.
- Rollback: `menu_backend = "wgpu"` e reiniciar o `--menu`.

**Onda 5, depois de 1 a 2 semanas:** tirar a feature `wgpu`. Opcional: P3 (`present_with_damage`). O
backdrop de 1×1 não está planejado.

**Testes puros novos (cerca de 30):**
- `pageout_ranges`: 8 casos;
- `menu_backend`: 5;
- `child_backend_env`: 2;
- `icon_raster`: alvo de SVG quadrado, largo, alto e zero; alvo de PNG igual a `downsample_target`,
  inclusive o `None`; símbolo não pré-rasterizado; id estável entre clones;
- `coalesce_damage`: 0, 1 e 2 retângulos intactos; com 3 ou mais, a união contém todos;
- `clear_last`: propriedade com retângulos aleatórios, inclusive fora dos limites e máscara recriada, sempre
  igual a uma máscara nova com o último retângulo;
- alfas dos anéis da sombra: monotônicos e iguais à fórmula, nos temas claro e escuro.

**Roteiro manual** (o que não se mede sem mouse):
1. Rolar "Todos" na lista e na grade, com roda e touchpad, por 10 s, observando engasgos com
   `pidstat -p PID 1`. Aceite: menos de 60% de um núcleo.
2. Abrir o popup de contexto, arrastar para fixar, abrir a prévia por 30 s e redimensionar o menu pela
   alça.
3. Trocar o tema claro/escuro com o menu aberto.
4. Abrir e fechar 20 vezes rapidamente.
5. Esperar 10 s escondido e reabrir (pageout).
6. Usar o rollback (`menu_backend = "wgpu"`) e voltar.
