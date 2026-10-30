# Custo de execução do menu Iniciar: medições e propostas (2026-10-22)

Escopo: apenas medição e leitura. Nenhum arquivo do repositório foi alterado. Os scripts e os logs brutos
estão em `target/perf/`: `measure.sh`/`run1.log`, `measure2.sh`/`run2-wgpu.log`, `ab2.sh`/`run3-ab.log`,
`idle.sh`, `procs.sh`, `threads.sh`, `wchan.sh` e `restore.sh`.
Binário medido: `~/.local/bin/cosmic-ext-applet-start` (build de 19:57). Ao final, o `--menu` normal (wgpu)
foi restaurado e deixado escondido (pid 2221690).

Ressalva sobre a carga: durante a rodada `run1`, outras sessões compilavam (load entre 11 e 53). Os números
de memória, de wakeups e de CPU do próprio processo não dependem disso. Já o A/B de CPU por abertura
(`run3`) e o repouso (`idle.sh`) foram refeitos com load entre 2,2 e 3.

## 1. Números medidos (antes)

### Processos

| Processo | VmRSS | RssAnon | RssFile | RssShmem | PSS | Threads | fds | Mapas de GPU | VRAM |
|---|---|---|---|---|---|---|---|---|---|
| Botão ×3 (tiny-skia) | 8,1–9,6 MB | 0,02–1,2 MB | 8,1 MB | 0 | 1,5–2,9 MB | 4–5 | 32 | 0 | 0 |
| `--menu` wgpu **escondido** | **110 MB** | 22 MB | 78 MB | 10 MB | **75 MB** | 10 | 56 (24 nvidia) | 95 | **19 MiB** |
| `--menu` wgpu visível | 127 MB | 39 MB | 78 MB | 10 MB | 92 MB | 14–15 | 67 | 95 | **107 MiB** |
| `--menu` wgpu com prévia (Brave) | 128 MB | 40 MB | 78 MB | 10 MB | 93 MB | 14 | 67 | 95 | n/m |
| `--menu` tiny-skia **escondido** | **29 MB** | 7,3 MB | 21,8 MB | 0 | **25 MB** | 5 | 28 | **0** | **0** |
| `--menu` tiny-skia visível | 80 MB | 20,6 MB | 21,7 MB | **37,5 MB** (buffers shm da tela cheia) | 57 MB | 8–9 | 35 | 0 | 0 |

Os botões estão quase todos em swap (cerca de 5,4 MB cada). Fora do escopo, mas medido de passagem: cada
`cosmic-ext-applet-taskbar` roda 36 threads.

### CPU e wakeups

| Estado | CPU | Wakeups (ctx switches) | Quem acorda |
|---|---|---|---|
| wgpu escondido, 60 s (load 26) | 0,15% de 1 núcleo (1,5 ms/s) | **104/s** | tid sem nome (`cosmic-ext-appl`, em `futex_wait`, criado depois das threads `[vk*]`): **thread do driver NVIDIA**, cerca de 100 Hz |
| wgpu escondido, 30 s (load 3, repetido) | 0,07% (0,7 ms/s) | **120/s** | a mesma thread com 100/s, mais `[vkps] Update` com 20/s |
| tiny-skia escondido, 60 s | **0** | **0** | nenhuma |
| wgpu visível parado, 20 s | 0,6% | 271/s | `[vkps] Update` 142/s, thread NVIDIA 100/s, principal 10/s |
| tiny-skia visível parado, 20 s | 0,2% | 33/s | principal e Wayland |
| wgpu com a prévia aberta (refresh de 1 s) | **2,75%** | 345/s | `start-menu-capture` 1,15% e principal 1,45% (recaptura a cada segundo) |

O tiny-skia escondido dá 0 wakeups com o mesmo código Rust. Isso prova que nenhuma subscription, timer ou
thread nossa acorda com o menu escondido. A leitura do código confirma: drag, resize e prévia ficam em
`Subscription::none()` quando ociosos, `report()` dos toplevels sai cedo com `WINDOWS_SHOWN=false`, e D-Bus e
Wayland ficam em `epoll`. Todo o custo em repouso vem do backend wgpu/NVIDIA. Isso bate com o que o commit
e3ba73a já registrava (cerca de 1 ms/s).

### Abertura

| Medida | wgpu | tiny-skia |
|---|---|---|
| CPU por ciclo abrir + 2 s parado + fechar (10 ciclos, load 2,5, duas rodadas) | **112 / 121 ms** | **136 / 151 ms** (cerca de 27 ms a mais) |
| Toggle até o log `surface size` (configure, 1 amostra, load alto) | 215 ms | 85 ms |

A latência da linha de baixo é **hipótese**: há só uma amostra de cada, sob carga, e ela mede o configure,
não o primeiro quadro.

### Vazamento e liberação

- **Esconder libera:** RssAnon cai de 39 para 22 MB (wgpu) e de 20,6 para 7,3 MB (tiny-skia). A VRAM cai de
  107 para 19 MiB. As miniaturas também são liberadas: escondido depois da prévia fica em 22,7 MB, igual à
  base.
- **35 ciclos:** RssAnon sobe de 23,0 para 25,3 MB e Shmem de 10,0 para 11,4 MB (wgpu). No tiny-skia, RssAnon
  sobe de 8,8 para 11,1 MB. Nos dois casos são cerca de **60 KB por abertura**, sem platô visível em 35
  ciclos. É **hipótese**: pode ser fragmentação do malloc ou crescimento de caches de ícones e fontes, e não
  está provado que seja vazamento.

### Journal

Em 24 h, **5.447 de 5.453** linhas do menu são avisos do `usvg` (`Failed to parse marker-* value: 'none'`,
3 por SVG analisado), em rajadas a cada abertura e rolagem. Isso indica que SVGs são reanalisados a cada
exibição. Na janela de medição foram 252 linhas para cerca de 32 aberturas.

## 2. O que mais pesa

1. **O backend wgpu do `--menu` em repouso.** Sozinho ele explica 81 MB de RSS (110 contra 29), 50 MB de PSS,
   19 MiB de VRAM e de 100 a 120 wakeups/s de uma thread do driver NVIDIA, 24 h por dia. O menu fica escondido
   quase o tempo todo.
2. **O wgpu visível:** 107 MiB de VRAM, pela superfície de tela cheia (3440×1440) em swapchain.
3. **A prévia aberta:** 2,75% de um núcleo enquanto o cartão está visível, por causa da recaptura a cada 1 s.
4. Os botões, as subscriptions, o tokio e o trim já estão otimizados. Não há ganho relevante a buscar ali.

## 3. Propostas ranqueadas (ganho / risco / esforço)

| # | Proposta | Ganho | Risco | Esforço | Arquivos |
|---|---|---|---|---|---|
| 1 | Silenciar o `usvg` no filtro padrão (`warn,usvg=error,cosmic_ext_applet_start=info`) | **Medido:** cerca de 5,4 mil linhas a menos por dia no journal. CPU desprezível. | Nenhum. Sem mudança visível. | 1 linha | `src/main.rs` |
| 2 | **tiny-skia também no `--menu`** (`set_var("ICED_BACKEND","tiny-skia")` no ramo `--menu`, guardando o valor original em `BACKEND` para os filhos, como o botão já faz) | **Medido** em repouso: RSS de 110 para 29 MB, PSS de 75 para 25 MB, VRAM de 19 para 0 (visível: de 107 MiB para 0), wakeups de 120/s para 0, CPU de 0,7–1,5 ms/s para 0, threads de 10 para 5. Contrapartida medida: cerca de 27 ms de CPU a mais por abertura e 37,5 MB de shm enquanto visível. | **Muda o comportamento visível:** a rolagem da lista ocupava 60% de um núcleo e engasgava pela CPU (commit e3ba73a; não refeito aqui). No menu não há blur nem sombra de GPU; a transparência funciona nos dois backends. | 2–3 linhas | `src/main.rs` |
| 3 | **Híbrido:** manter o wgpu, mas encerrar o processo `--menu` depois de N minutos escondido. O botão e o Win+X já relançam quando não há menu rodando (`toggle_running`/`tools_running`). | **Estimado:** repouso de longo prazo igual a 0 (os mesmos 110 MB e 120 wakeups/s somem), mantendo a rolagem pela GPU. | **Muda o comportamento visível:** a primeira abertura depois do ocioso vira partida a frio (não medida; **hipótese** de algumas centenas de ms). Perde estado em memória (resultado de desinstalação pendente). Exige um timer one-shot escondido. | Médio | `src/menu/application.rs`, `src/menu.rs` (hide) |
| 4 | Investigar o custo de rolagem no tiny-skia (por exemplo, dano da superfície de tela cheia, cache de SVG reanalisado; ver os avisos do `usvg`) para tornar a #2 viável sem engasgo | **Estimado.** Se resolver, a #2 fica sem contrapartida. | Baixo (investigação) | Médio a alto | `menu/pages.rs`, `tiles.rs`, superfície em `menu.rs` |
| 5 | Prévia: refresh de 1 s para 2–3 s, ou recapturar só a janela sob o ponteiro | **Medido:** 2,75% de um núcleo hoje; a redução seria proporcional (**estimado**). | **Muda o comportamento visível:** miniatura menos "ao vivo". | Pequeno | `src/menu/preview.rs` (`REFRESH`) |
| 6 | Superfície do tamanho do menu em vez da tela cheia | **Estimado:** VRAM visível de 107 para cerca de 10 MiB (wgpu) ou shm de 37,5 para cerca de 4 MB (tiny-skia). | **Muda o comportamento visível:** o clique no papel de parede deixa de fechar o menu (o código explica a escolha). Fecharia por perda de foco. | Médio | `src/menu.rs` (`show`) |
| 7 | `load_recents` a cada abertura percorre cerca de 1,4 mil entradas (Downloads 452, Imagens 861) e o xbel: guardar em cache pelo mtime das pastas, como `apps_signature` | **Estimado:** poucos ms por abertura (**hipótese**, não medido). | Baixo (sem mudança visível) | Pequeno | `src/menu/recent_files.rs` |
| 8 | Suspeita de ~60 KB por abertura: rodar 200 ciclos antes de agir | Desconhecido | Nenhum | Pequeno | (medição) |

Ponto de equilíbrio da #2: o wgpu gasta cerca de 0,7–1,5 ms/s escondido, ou seja, 40–90 ms por minuto. O
tiny-skia gasta cerca de 27 ms a mais por abertura. O tiny-skia só perde em CPU acima de 2 a 3 aberturas por
minuto, sem contar a rolagem.

Já aplicadas e conferidas, não mexer: botões em tiny-skia (0 mapas de GPU, PSS de 1,5 MB),
`TOKIO_WORKER_THREADS=1` (1 worker; as threads de `spawn_blocking` somem depois de cerca de 10 s),
`apps_signature`, `malloc_trim` no hide, miniaturas e memfd liberados no hide, nenhuma subscription ativa
com o menu escondido.

## 4. O que não foi medido, e por quê

- **Energia (RAPL):** o arquivo é legível (pacote de 41–86 W). Outras sessões compilavam (load de 2,5 a 53) e
  o ruído, de ±10 W, é muito maior que o efeito esperado. A estimativa de que 100 wakeups/s custam menos de
  0,1 W é **hipótese**. O resultado é "não medível" neste momento.
- **Tempo de `load_apps`:** não instrumentei nem compilei uma cópia. Um build completo competiria com outra
  sessão que estava compilando sondas em `target/exp-cod` (o próprio diretório de experimento já estava em
  uso) e contaminaria as medições. `strace` e `ltrace` não existem no host, e `perf` está bloqueado
  (paranoid=2, ptrace_scope=1). Em repouso o `apps_signature` já evita relê-los.
- **Tempo até o primeiro quadro:** não há span de quadro. Só foi medido o configure (`surface size`).
- **Custo de rolagem tiny-skia contra wgpu:** exige entrada de mouse, e não há ydotool. O valor de 60% vem do
  commit e3ba73a.
- **VRAM no estado de prévia e no tiny-skia:** `nvidia-smi` lista só processos com contexto de GPU.
- **Prévia de outros apps:** as tentativas com Claude e Antigravity não abriram a prévia, porque o app_id não
  bateu. Valem como "visível parado".
