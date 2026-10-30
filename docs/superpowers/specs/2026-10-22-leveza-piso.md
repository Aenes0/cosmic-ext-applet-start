# Leveza — o piso de um cliente Wayland layer-shell (experimento)

Data: 2026-10-22 · Pergunta: reescrever o menu (`--menu`, Rust/libcosmic/iced) numa linguagem ou pilha
"mais econômica" compensa? Resposta curta: **não. O peso está na pilha, quase nada na linguagem, e o
caminho incremental chega perto do piso.**

Tudo foi medido nesta máquina (CachyOS, COSMIC, NVIDIA 615.78, 32 núcleos, zram de 31 GB, carga
variável de 10 a 25 por causa de outras compilações). Os projetos descartáveis estão em
`target/exp-piso/` (`a-rust/`, `b-c/`, `c-cosmic/`, `measure.sh`, `parse.sh`, `runs/*.log`).
O repositório não foi alterado.

## Método

Cada cliente abre uma superfície layer-shell (`top`, sem região de entrada, sem foco de teclado,
porque o teste não pode atrapalhar o usuário). A superfície tem um buffer `wl_shm` ARGB de 874×920 (ou a
tela inteira com `FULL=1`, que dá 3440×1396 como no menu real), um retângulo arredondado e duas linhas
de texto em português com acentos. Ela fica **visível** por 34 s e depois é **escondida**: a superfície
e o buffer são destruídos, mas a conexão continua aberta. O `measure.sh` lê `/proc/<pid>/smaps_rollup` e
`/proc/<pid>/status` 3 s após a partida (visível), 4 s após esconder (escondido) e de novo 30 s depois.
As trocas de contexto voluntárias de **todas** as threads são somadas em janelas de 30 s. O "1º quadro"
vai do `fork` até o `done` do frame callback em A/B. Em C é aproximado pela primeira chamada a
`view_window`, então é um limite inferior (marcado com `~`). Valores em MB (1 MB = 1024 kB) e
repetições separadas por `/`.

- **A, Rust mínimo**: `smithay-client-toolkit 0.20`, sem iced, com `tiny-skia 0.11` e `cosmic-text 0.19`
  (shaping completo via harfrust e uma fonte, Open Sans), seat e teclado com xkbcommon, LTO fat e strip.
- **B, C puro**: libwayland-client, `wlr-layer-shell` gerado pelo `wayland-scanner`, FreeType
  (uma fonte, **sem shaping nem kerning**) e xkbcommon, com `-Os -flto` e strip, linkado dinamicamente.
- **C, a pilha**: um app libcosmic com as **mesmas features e patches** do menu, uma superfície do mesmo
  jeito que o menu (`get_layer_surface`/`destroy_layer_surface`), um `container` arredondado e dois
  `text`. Roda com `ICED_BACKEND=tiny-skia` ou com o padrão (wgpu).

## Tabela

| Cliente | Estado | RSS | PSS | Anon | File | Shm | Threads | Acordadas/s | Binário (strip) | 1º quadro |
|---|---|---|---|---|---|---|---|---|---|---|
| A Rust + uma fonte | visível | 9,4 | 5,1 | 1,0 | 5,4 | 3,1 | 1 | 0 | 2,9 MB | 52/63/86 ms (24 num 4º) |
| A Rust + uma fonte | escondido | **6,4** | **3,6** | **1,0** | 5,4 | 0 | 1 | **0** | | |
| A Rust sem texto | escondido | 4,7 | 2,0 | 0,9 | 3,8 | 0 | 1 | 0 | | 18/45 ms |
| A Rust + todas as fontes | escondido | 6,8 | 4,0 | 1,4 | 5,4 | 0 | 1 | 0 | | 52/55/58 ms |
| A Rust, tela cheia | visível | 24,6 | 12,7 | 1,0 | 5,3 | **18,3** | 1 | 0 | | 37/86 ms |
| B C + FreeType | visível | 9,1 | 4,0 | 0,8 | 5,2 | 3,1 | 1 | 0 | 31 KB (+ libs do sistema) | 27/28/33 ms |
| B C + FreeType | escondido | **6,0** | **2,5** | **0,8** | 5,2 | 0 | 1 | **0** | | |
| B C sem texto | escondido | 3,9 | 1,0 | 0,7 | 3,2 | 0 | 1 | 0 | | 23 ms |
| C libcosmic tiny-skia | visível | 29,3 | 22,5 | 5,9 | 17,3 | 6,1 (2 buffers) | 5 | 0–0,4 | 22,1 MB | ~26–30 ms |
| C libcosmic tiny-skia | escondido | **22,5** | **18,7** | **5,1** | 17,3 | 0 | 3 | 0–0,2 | | |
| C libcosmic tiny-skia, tela cheia | visível | 63,5 | 41,4 | 9,7 | 17,1 | **36,6** | 5 | 31,5 (uma execução) | | ~28 ms |
| C libcosmic wgpu (NVIDIA) | visível | 92/147 | 67/123 | 19/37 | 73/111 | 9,8 | 11 | **104** | | ~297–555 ms |
| C libcosmic wgpu (NVIDIA) | escondido | **92/147** | **67/122** | 19/36 | 73/111 | 9,8 | 8 | **104** | | |
| Menu real, wgpu (leitura de /proc) | escondido(?) | 97,5 | 73,1 | 22,1 | 75,4 | 9,8 | 10 | ≈100 (uma thread) | 25,7 MB (base.stripped) | — |

A dispersão é pequena em A, B e C-tiny-skia: menos de 0,15 MB de RSS entre repetições. No wgpu, a
primeira execução ficou em 147 MB e as três seguintes em 92 MB. O provável é o primeiro uso do cache de
shaders do driver (hipótese). O valor de tiny-skia do menu real, 29 MB, vem do enunciado: **eu não o
medi**.

### D — trim, arena, pageout (depois de esconder)

| Variante | RSS escondido | PSS | Anon |
|---|---|---|---|
| A + todas as fontes, sem nada | 6,8 | 4,0 | 1,42 |
| A + `malloc_trim(0)` | 6,6 | 3,8 | 1,29 |
| A + `MALLOC_ARENA_MAX=1` | sem diferença (uma thread só) | | |
| A + `madvise(MADV_PAGEOUT)` em todos os mapeamentos | 3,5 | 0,8 | 0,15 |
| B + PAGEOUT | 5,3 | 1,8 | 0,13 (as libs compartilhadas ficam, porque outros processos as mapeiam) |
| C tiny-skia, sem nada / + trim / + trim + arena1 | 22,4 / 22,4 / 22,6 | 18,6–18,8 | 5,0–5,1 |
| **C tiny-skia + PAGEOUT** (500 ms após esconder) | **6,0** (6,9 após 30 s) | **2,2** | 0,36 (0,56 após 30 s) |
| C wgpu + trim + arena1 | 91,4 | 66,1 | 18,9 |

O trim e a arena não mudam nada na pilha: os ~5 MB anônimos são dados vivos, não fragmentação. O
PAGEOUT leva a pilha inteira ao nível do piso em C. Duas ressalvas: o anon vai para o **zram** (continua
na RAM, só que comprimido), e as páginas de código voltam por page fault ao reabrir. **Não medi** a
latência de reabrir depois de um PAGEOUT.

### E — uma fonte vs todas as fontes do sistema

`fontdb::load_system_fonts` sobre 1001 arquivos (706 MB em `/usr/share/fonts`) custa **+0,43 MB de anon
(o índice de faces) e +~50 ms na partida**. O RssFile não muda, porque os arquivos são mapeados só
durante a varredura. Não medi o que o fallback em tempo de execução custa: um nome de app em CJK ou em
emoji carregaria e manteria uma fonte grande.

## O que os clientes mínimos NÃO fazem (por isso o piso é piso)

Não têm busca nem text_input (cursor, seleção, IME), rolagem, widgets, layout, temas e config do COSMIC
via D-Bus, nem lista de apps (desktop entries) e ícones (SVG/PNG, tema de ícones). Também não têm
miniaturas e captura de janelas, tooltips, popups, arrasto, grupos, redimensionamento, foco exclusivo de
teclado nem animação. B não faz shaping nem kerning. A faz shaping, mas de um único parágrafo. A e B
desenham um quadro e param, sem redesenhar na entrada do ponteiro.

## Conclusões

**1. O piso de um cliente Wayland vivo** (escondido, conexão aberta, teclado ligado) fica em ~6 MB de
RSS, dos quais ~4–5 MB são libc, xkbcommon e o próprio código. São 2,5 MB de PSS em C e 3,6 MB em Rust,
~1 MB anônimo, 1 thread e 0 acordadas/s. Visível, soma-se o buffer: 3,1 MB do tamanho do menu e
18,3 MB por buffer de tela cheia.

**2. Linguagem vs pilha.** C contra Rust, com o mesmo desenho, dá **0,4 MB de RSS e 1,2 MB de PSS** de
diferença (o código Rust estático contra as libs C compartilhadas do sistema) e zero de threads ou
acordadas. **A linguagem não pesa.** A pilha libcosmic/iced com tiny-skia, sem nenhuma lógica do menu,
custa **+16 MB de RSS escondida**. Desses, ~14 MB são páginas de código do binário de 22 MB (file-backed,
limpas e recuperáveis) e ~4 MB são anon. Há também +2 threads tokio. O wgpu no NVIDIA custa **mais ~70 MB
de RSS, +5 threads e ~104 acordadas/s sempre, mesmo escondido**. O primeiro quadro passa de ~30 ms para
300–550 ms. Com o `gdb`, confirmei que as 100 acordadas/s vêm de uma thread do driver
(`pthread_cond_timedwait` em `libnvidia-glcore`) que vive enquanto existir o VkDevice. O menu real mostra
o mesmo padrão: uma thread sem nome em `futex_wait` a ~100/s. Isso é um forte indício de que é a mesma
causa, mas não foi provado no processo real. Pela subtração, a lógica do menu custa ~6 MB tanto em wgpu
(97,5 − 92) quanto em tiny-skia (29 − 22,5). **Mais de 90% do peso é a pilha, e ~70% é só o wgpu.**

**3. Reescrever não se justifica pelos números.** Uma reescrita em Rust + tiny-skia + cosmic-text
partiria de ~6,5 MB e somaria os ~6 MB de dados e lógica do menu. Daria ~12 MB escondida (hipótese),
contra os 22–29 MB do caminho incremental sem PAGEOUT e ~6–7 MB com PAGEOUT (medido no baseline). Para
isso seria preciso reconstruir o que o iced dá pronto: layout, texto editável com IME, rolagem com
inércia, tooltips, popups, foco, tema e config do COSMIC, ícones SVG (resvg) e acessibilidade, além dos
14 módulos de UI (≈14,9 mil linhas em 62 arquivos). A estimativa, **não medida**, é de vários milhares
de linhas de toolkit antes da paridade, ou seja, semanas a meses, com risco de regressão em IME e
acessibilidade. C não muda nada material: −0,4 MB.

**Caminho incremental**, do maior ganho medido ao menor:
1. **Tirar o wgpu do `--menu`** (tiny-skia ou a feature fora): −70 MB de RSS, −5 threads e −104
   acordadas/s, com o 1º quadro caindo de ~0,4 s para ~30 ms. O risco é a rolagem em CPU numa superfície
   de 3440×1396, que foi o motivo da escolha pela GPU e **não foi medida aqui**.
2. **Liberar a memória ao esconder** (`MADV_PAGEOUT` ou `MADV_COLD` nos próprios mapeamentos, onde o
   menu já faz `malloc_trim`): 22,4 → 6,0 MB no baseline. O que falta medir é a latência de reabrir e o
   custo no zram.
3. **A superfície de tela cheia** custa 2 × 18,3 MB de shm com tiny-skia enquanto o menu está visível
   (36,6 MB contra 6,1 MB na superfície do tamanho do menu). Só pesa com o menu aberto. A troca já está
   documentada em `menu.rs`, porque é o clique no papel de parede que fecha o menu.
4. Trim e arena: **efeito nulo** (medido). Limitar fontes: ≤0,5 MB e ~50 ms (medido sem fallback).
   Pré-rasterizar ícones: **não medido**. A hipótese é que esse cache entra nos ~6 MB do menu.

## O que NÃO sei / não medi

- A memória de **GPU/VRAM** do wgpu e o que o compositor gasta para importar os buffers shm.
- A latência de **reabrir** depois de um PAGEOUT, e se o kernel já descarta as páginas de código por
  conta própria sob pressão. Elas são limpas, então provavelmente sim (hipótese).
- O desempenho de **rolagem** com tiny-skia na superfície de tela cheia.
- A decomposição dos ~6 MB próprios do menu (lista de apps, ícones, recentes, captura), que é uma
  estimativa por subtração. Os 29 MB do menu em tiny-skia vêm do enunciado.
- As 31,5 acordadas/s do C-tiny-skia em tela cheia visível vêm de uma única execução e não foram
  investigadas.
- O fallback de fontes em tempo de execução.
- **Linkagem estática e musl**: não há musl nem `libwayland-client.a`/`libfreetype.a` no host, então
  ficou só a dinâmica.
- O menu real foi lido só via `/proc` (rodava há 23 min e foi reinstalado às 20:57, durante o
  experimento). Não sei se estava visível na hora.
