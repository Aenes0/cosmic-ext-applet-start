# Menu Iniciar leve sempre (escondido e visível) sem perder fluidez: medições e caminho (2026-10-22)

> Escopo: só medição. Nada do repositório foi alterado além deste arquivo. Tudo foi feito em cópias:
> - `target/exp-leve`: base, com marcas de tempo e a sonda `src/menu/leve_probe.rs`;
> - `target/exp-leve2`: combinação final, sem a feature `wgpu`, com ícones pré-rasterizados, `pageout`
>   opcional e `[patch]` para o libcosmic local;
> - `target/exp-leve-cosmic`: cópia do libcosmic d4d71fd com dois patches no `iced_tiny_skia`.
>
> Build em `CARGO_TARGET_DIR=target/exp-leve-t`. Scripts e logs brutos ficam em `target/exp-leve-scripts/`:
> `live.sh`, `lat.sh`, `probes.sh`, `shots.sh` e `restore.sh`, mais `live-*.log`, `lat-*.log`, `probe-*.log`
> e `shot-*.png`/`diff-*.png`.
> No fim, o `--menu` instalado (wgpu) foi restaurado e deixado escondido.
>
> Rótulos: **[medido]** veio de sonda ou processo vivo nesta máquina. **[estimado]** é conta feita a partir de
> números medidos. **[hipótese]** não foi verificado.
>
> Carga: as sondas rodaram com load entre 1,9 e 2,6 (uma primeira rodada, com load 34, foi descartada para
> CPU). A dispersão aparece como mediana, p90 e faixa entre repetições.

## 1. Onde o CPU vai no tiny-skia (causa do engasgo da rolagem)

Leitura do fork (`iced/tiny_skia`, libcosmic d4d71fd), confirmada pela sonda:

1. **Dano fragmentado × máscara da superfície inteira.**
   - O compositor calcula o dano comparando as camadas quadro a quadro pela posição
     (`graphics/src/damage.rs:5`).
   - Ao rolar, o número de camadas muda (medido: oscila entre 30 e 45). As 7 camadas do tamanho da tela
     (backdrop, alças, prévia…) trocam de índice e entram inteiras no dano. Resultado: **30 a 49 retângulos
     por quadro**, cobrindo a tela toda.
   - Para **cada retângulo**, `Renderer::draw` (`lib.rs:79`) percorre **todas** as camadas. Para cada uma,
     `adjust_clip_mask` (`engine.rs:855`) faz `clip_mask.clear()` da **máscara inteira**: 3440×1400 = 4,8 MB.
   - Na prática, cada quadro de rolagem rasteriza a tela cerca de 30 vezes.
2. **Ícones refeitos a cada abertura.**
   - O renderizador é criado com a superfície e destruído no `hide`, e os caches morrem junto.
   - Além disso, `engine.trim()` roda depois de cada `draw` e descarta toda SVG e PNG que não foi desenhada
     naquele quadro (`vector.rs:228`, `raster.rs:232`).
   - Por isso, cada abertura relê, reanalisa e rerasteriza todas as SVGs (`usvg`) e redecodifica todos os PNGs.
     A sonda conta 27 avisos `usvg` por abertura, que são os 5,4 mil avisos por dia no journal. O custo é de
     0,76 ms por ícone **[medido]**: 39 SVG e 34 PNG.
3. **Sombra:** no tiny-skia ela **não existe**. `engine.rs:80` tem `if false` ("graphical glitches"). Não
   custa nada, mas é uma **diferença visível** em relação ao wgpu (ver a seção 5).
4. **Texto:** o glyph cache só é aparado a cada 300 draws. Não pesa.

## 2. Antes e depois por técnica

### Sonda de quadro

Renderizador tiny-skia real e headless, com o dano do compositor replicado (idade 2). Menu real: 73 apps,
superfície de 3440×1400 e menu de 874×920. "Rolagem" = `WheelScrolled` de 1 linha + `update` + `draw` +
rasterização com dano, 60 quadros (30 para baixo e 30 para cima). Foram feitas duas rodadas da base
(`base` e `base-b`) para medir a dispersão.

| Técnica (cumulativa) | Rolagem "Todos" Lista: mediana / p90 (ms por quadro) | Rolagem Grade: mediana / p90 | Grade com hover entre quadros: mediana / p90 |
|---|---|---|---|
| Base tiny-skia | **81,8–84,3 / 123–129** | **141–154 / 248–251** | 230–244 / 538–659 |
| + P1: dano = 1 retângulo envolvente quando há mais de 2 | 9,8 / 12,5 | 14,3 / 18,7 | 14,8 / 26,6 |
| + P2: máscara limpa só no retângulo anterior | 9,2 / 11,3 | 10,8 / 14,9 | 11,1 / 14,9 |
| + ícones pré-rasterizados (todos os pontos de uso) | **8,3–9,8 / 9,5–11,1** | **9,3–10,3 / 12,8–13,1** | 9,7–9,8 / 12,6–13,7 |
| Mesma combinação, superfície do tamanho do menu (906×952) | 7,0 / 9,1–9,4 | 8,3 / 9,9–10,6 | 9,0 / 11,9 |

| Abertura (renderizador novo, como no app) | Base (svg) | Ícones pré-rasterizados |
|---|---|---|
| `view` + layout (inclui o parse das SVGs) | 26–30 ms (73–76 ms na 1ª vez do processo) | **1,6–2,3 ms** |
| Primeiro quadro rasterizado: tela cheia / tamanho do menu | 33–36 / 26–29 ms | 19–23 / 17–20 ms |

### Processo vivo

Medido com `live.sh`/`lat.sh` (12 aberturas por variante). Binários:
- `bin-base`/`bin-base2`: o código do repositório, apenas com as marcas de tempo;
- `bin-leve`: `exp-leve2`.

As "acordadas" são trocas de contexto somadas de todas as threads em 10 s. Valores ≤ 0 indicam threads de
bloqueio terminando dentro da janela, ou seja, nenhuma acordada.

| Variante | Escondido: RSS / Anon / File / PSS (MB) | Threads e acordadas/s escondido | Visível: RSS / Shm (MB) | Toggle → `view` (mediana, faixa) | CPU por ciclo abrir+fechar |
|---|---|---|---|---|---|
| wgpu (hoje) | 166–168 / 38–42 / 117 / 131–133 | 12–13, 79–97/s | 183 / 10 + 107 MiB de VRAM (perf-runtime) | 89,7 ms (83–97) | 113 ms |
| tiny-skia base | 29,1–30,3 / 7,4–8,6 / 21,7 / 25,0–26,0 | 7–8 (5 após 10 s), 0 | 80 / 37,5 | 83,4 ms (82–92) | 133–149 ms |
| tiny-skia + `MALLOC_ARENA_MAX=1` | 29,1–29,8 / 7,2–8,0 / 21,8 / 24,9–25,6 | idem | 79 / 37,5 | — | 149 ms |
| **leve** (sem feature wgpu, P1+P2, ícones RGBA) | 28,9–29,8 / 8,1–8,9 / 20,8 / 24,8–25,7 | 5 após 10 s, 0 | 77,5 / 37,5 | **69,5 ms (67–75)** | 97–117 ms |
| **leve + `MADV_PAGEOUT` no hide** | **23,1–23,5 / 2,2–2,5** / 20,9 / **18,9–19,2** (swap em zram: 5,8–6,4) | 0 | 74,4 / 37,5 | 75,1 ms (53–81) | 134–143 ms |
| leve + THP desligado (`PR_SET_THP_DISABLE`) | 29,1–29,9 / 8,1–8,9 / 20,9 | 0 | 77,6 | 72 ms (59–115) | 97 ms |

Primeiro quadro na tela **[estimado]** = toggle→`view` mais `view`/layout e a rasterização da sonda:
- tiny-skia base: cerca de 83 + 27 + 35 ≈ **145 ms**;
- leve: cerca de 70 + 2 + 21 ≈ **93 ms**;
- wgpu: cerca de 90 ms mais o preparo na GPU, que não foi medido.

O intervalo show→configure (65–85 ms) domina a latência e independe do renderizador. A causa não foi
investigada (**[hipótese]**: ida e volta com o compositor mais a criação da superfície no iced).

### Itens pedidos, um a um

- **(1) Ícones pré-rasterizados.**
  - Custo: 57 ms para rasterizar 73 ícones num tamanho, fora da thread da UI e só quando os apps mudam.
    Memória: 224 KiB (28 px) + 479 KiB (41 px) = **+0,7 MB de anon permanente** (medido: anon escondido
    8,1 contra 7,4 MB).
  - Ganho: abertura sem parse (26–30 → 2 ms), 0 avisos `usvg`, rolagem até 10% mais barata.
  - Os outros tamanhos (16, 37, 48, 74) continuam em SVG: uso raro.
  - O handle precisa ser criado **uma vez** (em `load_apps`). `from_raster_pixels` gera um `Id::unique()` a
    cada criação, então criá-lo na `view` derrubaria o cache.
  - A versão em disco (mmap) foi descartada: só trocaria 0,7 MB de anon por RssFile **[estimado]**.
- **(2) Superfície.**
  - A tela cheia custa **37,5 MB de shm visível**: 2 buffers de 3440×1400×4, ambos preenchidos.
  - Um buffer do tamanho do menu custaria cerca de 6,9 MB **[estimado]** e 15–20% menos rasterização
    **[medido na sonda]**.
  - Leitura do fork:
    - o buffer é sempre do tamanho configurado;
    - não há `set_opaque_region`;
    - `input_zone` existe, mas só restringe a entrada;
    - todo `Window` do iced cria compositor e buffer;
    - o viewporter já é usado em toda layer surface, mas só para escala fracionária;
    - não há `single_pixel_buffer`.
  - Um backdrop de 1×1 esticado exige mudar o fork (softbuffer/compositor tiny-skia com buffer de 1×1 e
    `set_destination` da tela). Risco médio, ganho de cerca de 30 MB **só enquanto visível**. Não foi
    medido ao vivo.
  - Uma segunda layer surface transparente de tela cheia não resolve, porque também teria buffer de tela
    cheia **[hipótese]**.
- **(3) Memória ao esconder.**
  - `malloc_trim(0)` já existe.
  - `MALLOC_ARENA_MAX=1`: −0,2 MB, dentro do ruído.
  - THP desligado: 0 (hipótese refutada).
  - **`MADV_PAGEOUT` nos mapeamentos anônimos privados e no heap**, depois do trim: **−5,7 MB de anon**
    (8,1 → 2,4), PSS de 25 → 19 MB.
  - Volta com custo não mensurável: toggle→`view` de 75 ms (53–81) contra 69,5 (67–75), faixas sobrepostas.
    CPU por ciclo +20–40 ms, dentro do ruído de ±20.
  - O zram (32 GB, prioridade 100) guarda os 5,8 MB comprimidos. Ganho físico líquido de cerca de 4 MB
    **[estimado, razão de compressão não medida]**.
  - Caches do renderizador (fontes rasterizadas, SVG, imagens) já morrem com a superfície.
    `Arc<Vec<AppEntry>>` ≈ 120 KiB (perf-codigo): não vale descartar.
- **(4) RssFile e fontes.**
  - fontdb: **890 faces, 18 ms, +0,5 MB de anon** e 0,9 MB de RssFile transitório. O clone que o cache de SVG
    faz custa +0,17 MB.
  - Ficam residentes só 2 arquivos (OpenSans, 0,27 MB).
  - Limitar famílias exige patch em `graphics/text.rs:131` e arrisca fallback de glifos (emoji/CJK em nomes
    de apps), por no máximo 0,7 MB. **Não recomendado.**
  - Os **17,5 MB de RssFile são o `.text` do próprio binário** (14,3 MB de `.text` + 3,5 MB de `.rodata`).
    É o **mesmo arquivo** dos 3 botões do painel, então em produção as páginas são compartilhadas e divididas
    no PSS (não medido: o binário de teste era outro arquivo).
  - Sem a feature `wgpu`:
    - binário de 35,8 → **28,3 MB**;
    - RssFile −0,9 MB;
    - nada de libvulkan/nvidia carregado;
    - compila e roda (`transparent(true)` ok).
  - O crate `wgpu` **continua sendo compilado**, porque a feature `wayland` do libcosmic ativa
    `iced_wgpu/wayland` sem `?`.
- **(5) Threads.** Escondido ficam 5: principal, 2 `tokio-rt-worker` (runtime do app e do zbus), 1 sem nome
  (Wayland/calloop) e `start-menu-capture`. Todas em 0 acordadas/s. Não há timer residual. Nada a ganhar além
  de centenas de KB de pilha.
- **(6) Abertura.**
  - Ver a tabela de latência acima.
  - `load_apps` a frio leva 488 ms (perf-codigo) e roda uma vez por vida do processo, fora da UI. Os ícones
    acrescentam cerca de 114 ms só quando os apps mudam.
  - **Cache em disco das entradas: não recomendado** (o processo vive sempre).

## 3. Combinação recomendada e piso medido (`bin-leve`)

Combinação: tiny-skia + P1 + P2 (fork do libcosmic) + ícones RGBA de 41/28 px criados em `load_apps` + sem
a feature `wgpu`, e opcionalmente `MADV_PAGEOUT` no `Trim`.

- **Escondido:** RSS de **29 MB (23 com pageout)**, contra 166 MB do wgpu. PSS de 25 (19), anon de 8,1 (2,4).
  **0 acordadas/s, 0 CPU, 5 threads, 0 VRAM.**
- **Visível:** 77,5 MB (37,5 de shm). Fica em cerca de 47 MB **[estimado]** se o backdrop de 1×1 for feito.
- **Rolagem de "Todos":** de **82–154 ms** para **8,3–10,3 ms** por quadro, mediana; p90 ≤ 13 ms.
- **Abertura:** toggle→`view` de 69,5 ms. Primeiro quadro em cerca de 93 ms **[estimado]**.
- **Piso real:** anon de cerca de 2,4 MB + `.text` compartilhado. Abaixo disso, só encerrando o processo,
  o que foi descartado pelo usuário.

## 4. Ordem sugerida para produção

1. **Fork do libcosmic com P1 + P2.** Diff pequeno, em `iced/tiny_skia/src/window/compositor.rs` e
   `engine.rs`; ver `target/exp-leve-cosmic`. Propor upstream.
   - Só funcionou substituindo as 18 crates do repositório git (`[patch]` com caminho). Vendorizar só
     `iced_tiny_skia` esbarra na herança de workspace **[hipótese]**.
   - Teste: a sonda `leve_frames` (mediana < 15 ms) e a comparação de print: base tiny contra leve teve
     **0 px diferentes fora dos ícones**, ou seja, P1/P2 não mudaram nada do desenho.
2. **Ícones pré-rasterizados.** Reproduzir **exatamente** o caminho do renderizador:
   - SVG: `scale_to_width/height` sem centralizar;
   - PNG: `downsample_premultiplied` do `iced_graphics`.

   O protótipo usou CatmullRom e centralização e deu **5.133 px diferentes, só nas bordas de 13 ícones**
   (diferença mediana de 35/255). Critério de aceite: 0 px acima de 8 na comparação de print.
3. **`--menu` em tiny-skia e sem a feature `wgpu`**, com `ICED_BACKEND` no ramo `--menu`, como o botão já faz.
4. **Decidir a sombra** (ver a seção 5).
5. Opcional: `MADV_PAGEOUT` no `Trim`.
6. Opcional (fork, risco médio): backdrop de 1×1 com viewporter.

## 5. Riscos

- **Visual:** o tiny-skia **não desenha a sombra** do cartão (`blur_radius` 16, alfa 0,45).
  - Medido: 19.723 px de diferença fora do corpo do menu entre wgpu e tiny-skia.
  - É a única mudança visível. Opções: aceitar, ou emular com imagem pré-rasterizada atrás do cartão (cerca
    de 3,4 MB enquanto visível **[estimado]**). O anti-aliasing de texto também difere do wgpu (12 mil px
    dentro do cartão, sutil).
- **Correção:**
  - P2 guarda o último retângulo por endereço da máscara num `thread_local`, com proteção de limites contra
    máscaras recriadas.
  - P1 troca muitos retângulos por um envelope: nunca desenha menos que o necessário, só mais.
- **Compatibilidade:** manter um fork do libcosmic, ou reaplicar 2 patches a cada atualização.
- **Ainda acima de 6,9 ms (144 Hz):** a rolagem continua em 8–10 ms porque o dano posicional ainda cobre a
  tela. Um patch de diff alinhado por bounds levaria a cerca de 3–5 ms **[hipótese]**.
- **Scripts:** `pgrep -f 'applet-start --menu'` (em `target/prints/stopmenu.sh` e `target/perf/*.sh`) também
  casa shells de outras sessões que tenham esse texto na linha de comando; observei isso aqui. Use a
  conferência por `/proc/PID/exe` e `argv[1]` de `target/exp-leve-scripts/common.sh`.

## 6. O que não foi medido

- **Rolagem real com mouse e touchpad:** não há injeção de entrada. Valem a sonda de quadro e a taxa de
  eventos, também não medida.
- **CPU de rolagem no wgpu** para comparação direta.
- **Primeiro quadro na tela:** só toggle→`view` mais a soma da sonda.
- **Razão de compressão do zram.**
- **PSS com o binário compartilhado pelos botões.**
- **Backdrop de 1×1 ao vivo.**
- **Energia:** pelo mesmo motivo do perf-runtime.
