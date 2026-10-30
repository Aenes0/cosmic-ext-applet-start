# Leveza do menu: referências (Windows, lançadores Linux) e técnicas para Linux/Wayland

Data: 2026-10-22. Escopo: pesquisa e medição. Nenhum código foi alterado.
Complementa `2026-10-22-perf-runtime.md`, que tem as medições do próprio applet (wgpu × tiny-skia).

**Rótulos usados neste documento**

- **[PUB]**: número publicado, com fonte e data.
- **[MED]**: medido aqui no host em 22/10/2026.
- **[EST]**: estimativa minha, com a conta mostrada.
- **[?]**: não foi possível confirmar.

A máquina não tem Windows (removido em 21/10), então nenhum número do Windows foi medido aqui.

Host das medições [MED]:

- CachyOS, kernel 7.2.11, 32 threads lógicas, 31,2 GiB de RAM.
- zram0: zstd, 32 GiB, taxa atual de 3,07:1 (7,59 GB → 2,47 GB). `swappiness=150`, `page-cluster=0`.
- THP `always`, glibc 2.44, NVIDIA 615.78.08.
- 1001 fontes (`fc-list`), 706 MB em `/usr/share/fonts`.

---

## 1. Windows: como o Iniciar fica barato quando escondido

### Arquitetura

- Desde o Windows 10 1903 (maio de 2019), o Iniciar roda num processo próprio, `StartMenuExperienceHost.exe`, que aparece como "Start" no Gerenciador de Tarefas. Antes ele rodava dentro do `ShellExperienceHost`. A Microsoft justificou a mudança pela confiabilidade: isolar o Iniciar das outras superfícies do shell. [PUB] BleepingComputer, 20/04/2019; Windows Latest, 21/04/2019.
- O processo é um app empacotado (UWP/SystemApps) e segue o ciclo de vida do PLM (Process Lifecycle Manager):
  - O Windows suspende o app "shortly after" ele sair da tela, ou seja, "within a few seconds".
  - Suspender para as threads do app: "the app's threads are stopped".
  - A memória continua residente "unless the operating system needs to reclaim resources".
  - [PUB] Microsoft Learn, *UWP app lifecycle*, ms.date 13/09/2023.
- O Gerenciador de Tarefas mostra o Iniciar como "Suspended", com a dica "This UWP process is suspended to improve system performance". [PUB] Microsoft Q&A (fórum, sem data precisa).
- **Trim ao suspender:**
  - Desde o 8.1, apps Direct3D/Direct2D são obrigados a chamar `IDXGIDevice3::Trim` no callback de suspensão. A chamada libera buffers internos de GPU, que contam na memória do app perante o PLM. [PUB] Microsoft Learn, *IDXGIDevice3::Trim*.
  - Ao ir para o segundo plano, "the UI frameworks flush cached textures and the video subsystem frees memory allocated on behalf of the app".
  - O app também recebe `AppMemoryUsageLimitChanging` e deve descartar a árvore de UI (`Window.Content = null` + `GC.Collect`).
  - [PUB] Microsoft Learn, *Reduce memory usage when your app moves to the background*, ms.date 08/02/2017, atualizado em 20/10/2022.
- **Compressão de memória:**
  - Páginas aparadas ("trimmed") vão comprimidas para o working set do processo de sistema em vez de ir para o disco.
  - Cada app UWP tem seu próprio "compression store".
  - Um app suspenso é retirado da memória inteiro, de uma vez, porque ninguém toca nas páginas dele.
  - [PUB] Petri, *Memory Compression in Windows 10 TH2*, 2015 (build 10525, era de preview; pode ter mudado).
- **Windows 11:**
  - As seções "All apps" e "Recommended" do Iniciar usam React Native for Windows. O resto é C++/XAML.
  - Usuários relatam pico de 30 a 70% de um núcleo ao clicar no Iniciar, em cerca de 50% das interações. [PUB] Winaero, atualizado em 10/03/2026; o número é de relato de usuários, não de benchmark.
  - A Microsoft anunciou uma reescrita em WinUI e admitiu que os apps nativos "hog RAM", mas "hasn't provided any specific figures". [PUB] PCWorld, 27/07/2026.
- **Busca:**
  - `SearchHost.exe` usa WebView2 com subprocessos.
  - Relatos no 24H2: 34 MB no SearchHost e 138 MB somando os itens de Search; outro usuário viu cerca de 500 MB com os filhos WebView2. [PUB-anedota] ElevenForum, thread *Stop SearchHost.exe from running?* (2025, data exata [?]).
  - O índice da busca fica num serviço separado (Windows Search), fora do processo do Iniciar.

### Números de memória e CPU do Iniciar

| Item | Número | Qualidade |
|---|---|---|
| `StartMenuExperienceHost` "uso típico" | 20–60 MB | [PUB-fraco] TechLasi (sem data e sem metodologia); >200 MB tratado como problema |
| `SearchHost` (24H2) | 34 MB; 138 MB somando Search; ~500 MB com WebView2 | [PUB-anedota] ElevenForum, 2025 |
| CPU ao abrir (Win11, React Native) | 30–70% de 1 núcleo, em ~50% das aberturas | [PUB-anedota] Winaero, 2026 |
| CPU e wakeups com o Iniciar suspenso | 0 (threads paradas pelo PLM) | [PUB] da semântica do PLM; não há medição publicada |
| Working set do Iniciar suspenso depois do trim e da compressão | [?] | Não há número publicado da Microsoft nem de terceiros |

**Conclusão honesta:**

- Não existe medição confiável e publicada do working set e dos private bytes do Iniciar.
- A faixa "20–60 MB" é a única que circula, e a fonte é fraca.
- O que é documentado é o mecanismo para custar perto de 0 quando escondido:
  1. Suspender as threads, o que dá 0 de CPU e 0 de wakeups.
  2. Aparar a GPU (`Trim`).
  3. Descartar a árvore de UI e as texturas.
  4. Comprimir as páginas restantes num store por app.
  5. Em último caso, terminar o app e relançá-lo depois.

---

## 2. Linux/Wayland: custo real de lançadores e menus

| Lançador | RSS | PSS | Threads | Wakeups/s escondido | Fonte | Como fica residente |
|---|---|---|---|---|---|---|
| **cosmic-launcher** (sistema, wgpu) | 51,3 MB | 43,7 MB (anon 14) | **74** (64 `tokio-rt-worker`) | **127** | [MED] | Residente; NVIDIA mapeada (81 mapas); 6 MB de THP |
| **cosmic-app-library** (sistema) | 30,8 MB | 24,8 MB | 75 | **114** | [MED] | Residente |
| pop-launcher (serviço de busca do cosmic-launcher) | 4,7 MB | 1,8 MB | 2 | – | [MED] | Residente |
| pop-launcher plugin `cosmic_toplevel` | 13,0 MB | 10,1 MB | 3 | – | [MED] | Residente |
| cosmic-panel-button (Workspaces, ×3) | 6,4 MB | 0,6 MB (anon 16 kB, **swap 7,9 MB**) | 35 | – | [MED] | O kernel já mandou quase tudo para o zram |
| cosmic-notifications | 6,8 MB | 3,1 MB | **1** | – | [MED] | Residente |
| cosmic-applet-status-area | 5,4 MB | 1,0 MB | 33 | – | [MED] | Residente |
| **nosso `--menu` wgpu escondido** | 100–110 MB | 75 MB (anon 22,6) | 10 | 124 | [MED] (repetido) | ~45 MB de RssFile só de libs NVIDIA + 9,9 MB de `/dev/nvidiactl` |
| nosso `--menu` tiny-skia escondido | 29 MB | 25 MB | 5 | 0 | [MED] em perf-runtime | – |
| nosso botão (tiny-skia) | 8,0 MB | **1,47 MB** | 4 | 0 | [MED] | Binário compartilhado: 4,2 MB de RssFile, dividido no PSS |
| plasmashell (Plasma 6.1) | 200–300 MiB | – | – | – | [PUB-anedota] KDE Discuss, 2024 | Residente (shell inteiro, inclui o Kickoff) |
| gnome-shell | 51 MB (2011); ~0,6–1,2 GB (2019) | – | – | – | [PUB-anedota] Arch BBS 2011; Ubuntu Discourse 2019 | Compositor + shell + overview no mesmo processo |
| Ulauncher (Python + WebKit) | 200–400 MB | – | – | – | [PUB-fraco] README do dlauncher (parte interessada) | Daemon residente |
| foot (cliente de servidor) | 2,32 MB por cliente; servidor 52 MB | – | – | – | [PUB-anedota] FreeBSD forums | – |

Não estão instalados e não foram medidos [?]: fuzzel, wofi, rofi, Albert, nwg-drawer e eww. Por arquitetura:

- fuzzel, wofi e rofi **não ficam residentes**. São processos de vida curta, lançados por atalho, então custam 0 escondidos e pagam a partida a frio a cada abertura.
- No fuzzel, a única evidência é um valgrind com cerca de 21 KB "in use at exit" (commit de correção de vazamento), que não é medida de RSS.

**Lições:**

1. O próprio COSMIC já paga 51 + 31 MB de RSS e cerca de 240 wakeups/s nos dois lançadores de sistema. O motivo é o mesmo nosso: wgpu/NVIDIA, mais tokio com 64 workers.
2. Com zram e `swappiness=150`, o kernel já faz um "suspender + comprimir" passivo nos processos ociosos. O cosmic-panel-button ficou com 16 kB de anon residente e 7,9 MB no zram, e o xdg-desktop-portal-cosmic com 112 MB no zram. A diferença para o Windows é que isso é preguiçoso (só sob pressão ou por idade) e não para as threads. Quem zera os wakeups é o código.
3. O piso prático de um processo libcosmic vivo é de **5–8 MB de RSS** e **1–3 MB de PSS** (status-area, notifications e o nosso botão, todos [MED]).

---

## 3. Técnicas Linux equivalentes a "suspender + trim" sem matar o processo

Medição-chave [MED]:

- 64 MiB anônimos, 50% aleatórios, `madvise(MADV_PAGEOUT)` para o zram (zstd): **122 ms** (≈1,9 ms/MiB). O RssAnon foi de 69 MB para 3,6 MB.
- Repaginar tocando cada página: **83 ms, 5,1 µs por página (≈1,3 ms/MiB)**. Foi serial, porque `page-cluster=0` desliga o readahead do swap.
- Tocar as mesmas páginas quentes: 1,7–2,0 ms.
- Duas rodadas idênticas.

Ranking por ganho esperado × custo × risco, para o `--menu` escondido:

| # | Técnica | Ganho esperado | Custo | Risco / observação |
|---|---|---|---|---|
| 1 | **Backend sem GPU (tiny-skia) no `--menu`** | [MED] RSS 110 → 29 MB, PSS 75 → 25, wakeups 124 → 0, VRAM 19 → 0, threads 10 → 5 | 2–3 linhas | A rolagem usa CPU (perf-runtime #2/#4). É o equivalente direto do `Trim` mais o "threads paradas" do Windows: sem driver, não há thread acordando. **Domina todas as outras técnicas.** |
| 2 | Superfície do tamanho do menu em vez da tela cheia (3440×1440) | [EST] shm visível 37,5 MB → ~3,7 MB (640×720×4 B×2 buffers) | Médio | Muda o fechamento por clique fora (perf-runtime #6) |
| 3 | Descartar a árvore de UI e os caches (ícones, SVG rasterizado, miniaturas) ao esconder e reconstruir ao mostrar | [EST] RssAnon 7,3 MB → 2–4 MB | Médio | Primeira renderização mais cara. É o `Window.Content = null` do UWP. `malloc_trim` no hide **já existe** |
| 4 | `MADV_PAGEOUT` no heap e nos anônimos próprios N s depois de esconder (percorrer `/proc/self/maps`, só `[heap]` e anônimos privados; nunca pilha nem memfd/shm) | [EST] Pss_Anon de ~7 → <0,5 MB; PSS do tiny-skia escondido de ~25 → ~18 MB (o RssFile continua) | Baixo (~40 linhas) | Repaginação [MED] de ~1,3 ms/MiB: 7 MB ≈ 9 ms, 22 MB ≈ 29 ms no 1º quadro. Mitigar com pré-toque assíncrono no `pointer-enter` do botão. Exige swap/zram (há). O kernel já faz isso sozinho com o tempo, então o ganho em regime é **cosmético e antecipado** |
| 5 | `M_ARENA_MAX` = 1–2 (`mallopt` no `main` ou `MALLOC_ARENA_MAX`) e `M_TRIM_THRESHOLD` baixo | [EST] 0–3 MB de anon (menos arenas por thread, menos fragmentação; perf-runtime viu +2,3 MB em 35 ciclos) | 2 linhas | Baixo. Alternativa: mimalloc com `purge_delay` (padrão de 1000 ms na v3 [PUB] docs do mimalloc), que devolve sozinho, mas troca de alocador |
| 6 | Menos threads: tokio `current_thread` (ou 1 worker) e nada de worker ocioso | [EST] RSS −0,1 a −0,5 MB (pilhas tocadas); threads 5 → 2–3 | Baixo | Só vale para a meta de threads. A pilha virtual de 2 MiB do Rust não conta em RSS |
| 7 | Binário menor: `opt-level="s"`, `lto="fat"`, `codegen-units=1` (`.text` hoje de 18,5 MB, arquivo de 35,7 MB) | [EST] RssFile −1 a −4 MB | Rebuild | O RssFile é **limpo e descartável** pelo kernel sem swap, e é dividido no PSS entre os 4 processos do mesmo binário. Ganho real em PSS: ≤1 MB. `strip` não muda o RSS (o debug não é mapeado) |
| 8 | Fontes | [MED] só 2 arquivos de fonte mapeados, 260 kB de Rss | – | **Não é o problema de RSS.** Os metadados do fontdb para 1001 arquivos ficam no anon ([EST] ~1–2 MB) e custam tempo de partida. Hipótese de ganho: restringir as famílias carregadas |
| 9 | `PR_SET_TIMERSLACK` / timerfd coalescido | 0 escondido (o tiny-skia já tem 0 wakeups) | Baixo | Útil só **visível** (33 wakeups/s parado) |
| 10 | `PR_SET_THP_DISABLE` | [MED] o `--menu` tem `AnonHugePages` = 0 | – | Sem ganho hoje. O cosmic-launcher tem 6 MB de THP |
| ✗ | cgroup v2 `cgroup.freeze` | 0 de CPU (já é 0) | Alto | Um processo congelado não lê o socket Wayland nem o D-Bus. Alguém de fora (o botão) teria que descongelar antes de cada toggle, e o compositor pode desconectar o cliente se o buffer encher. Hoje o `--menu` está no cgroup `flatpak-session-helper.service`: congelar pegaria outros processos. [PUB] kernel cgroup-v2 doc: "stop and stay stopped until … explicitly unfrozen" |
| ✗ | `mlock`, `MAP_POPULATE` | Negativo | – | Prende ou infla o RSS. Contraindicado |
| ✗ | `LD_BIND_NOW` | ~0 | – | Afeta só o tempo de partida e as páginas de GOT/PLT. Irrelevante |
| ✗ | `MADV_FREE` / `MADV_DONTNEED` no heap vivo | – | – | Destrutivos: **só** valem para memória que o alocador já liberou, que é o que o `malloc_trim` faz |

Base das técnicas [PUB]:

- `MADV_COLD` e `MADV_PAGEOUT` existem desde o Linux 5.4 e não são destrutivos. Foram criados para o Android "reclaim apps as soon as they entered the cached state" (LWN 2019; man7 madvise, man-pages 6.19, 08/02/2026).
- `malloc_trim` libera todas as arenas desde a glibc 2.8 (man7 malloc_trim, 08/02/2026).

---

## 4. Metas absolutas propostas (medíveis com `/proc/<pid>/smaps_rollup`, `/proc/<pid>/task/*/status`, `nvidia-smi`)

Justificativa:

- O piso medido de um cliente libcosmic vivo neste host é de 5–8 MB de RSS e 1–3 MB de PSS.
- O Windows suspenso tem 0 de CPU.
- O Iniciar visível do Windows usa de 20 a 60 MB [PUB-fraco].

| Métrica | Hoje (wgpu) | Hoje (tiny-skia) | **Meta: escondido** (5 s depois do hide) | **Meta: visível parado** |
|---|---|---|---|---|
| RSS | 110 MB | 29 MB | **≤ 12 MB** (alvo 8) | **≤ 25 MB** |
| PSS | 75 MB | 25 MB | **≤ 6 MB** (alvo 3) | **≤ 15 MB** |
| Pss_Anon (privado de verdade) | 22,6 MB | 7,3 MB | **≤ 1 MB** com pageout; ≤ 3 MB sem | ≤ 10 MB |
| shm (buffers) | 10 MB | 0 / 37,5 visível | **0** | **≤ 4 MB** |
| Threads | 10 | 5 | **≤ 3** | ≤ 4 |
| Wakeups/s | 124 | 0 | **0** | ≤ 5 parado (o resto só com entrada) |
| CPU | 0,7–1,5 ms/s | 0 | **0 ms/s** | ≤ 0,5 ms/s parado |
| VRAM | 19 / 107 MiB | 0 | **0** | **0** |
| Clique → 1º quadro | n/m | n/m (+27 ms de CPU por abertura) | – | **p95 ≤ 50 ms** quente; ≤ 80 ms depois do pageout |

**O que "muito mais leve que o Windows" pode significar, de forma honesta:**

- **100× é fisicamente impossível em RSS.** 100× de 20–60 MB dá 0,2–0,6 MB. Só a libc mapeada já tem 1,7 MB de RSS [MED], e um cliente Wayland em Rust/libcosmic não fica abaixo de cerca de 5 MB de RSS.
- O que é possível:
  - **≥ 10× em PSS** escondido: ≤ 3–6 MB contra 20–60 MB [EST × PUB-fraco].
  - **~20–60× em memória privada** (Pss_Anon ≤ 1 MB com pageout).
  - **Paridade em CPU e wakeups (0 = 0)**, porque o Windows suspenso também é 0.
  - **∞× em VRAM** (0 contra a composição DWM/XAML).
- O que nem o Windows mede de forma comparável: PSS (o Windows reporta working set, que conta DLLs compartilhadas por inteiro, como o RSS). Comparar RSS com working set é o mais justo, e aí a meta realista fica em **3–7× (≤ 8–12 MB contra 20–60 MB)**.
- Trocar de linguagem ou pilha (C + wayland-client + pixman, estilo fuzzel/foot) daria um piso de [EST] ~3–4 MB de RSS e <1 MB de PSS, só cerca de 2× melhor que a meta acima, a um custo de reescrita total. **Não compensa.** O ganho grande (110 → ~8 MB) está no backend e no descarte ao esconder, não na linguagem.

---

## 5. O que não foi confirmado

- Working set e private bytes do `StartMenuExperienceHost` suspenso ou visível, em medição confiável: não existe nem da Microsoft nem de benchmark. A faixa de 20–60 MB vem de fonte fraca.
- O que o PLM do Windows faz exatamente com o Iniciar (se ele é suspenso como um app comum ou fica isento). Só o rótulo "Suspended" no Gerenciador é relatado.
- RSS medido de fuzzel, wofi, rofi, Albert, nwg-drawer, eww e GNOME 46/47: não estão instalados, e não há número publicado confiável.
- Tempo até o 1º quadro do nosso menu e do cosmic-launcher: não medido (falta instrumentação de clique → commit).
- O efeito real do `MADV_PAGEOUT` no próprio `--menu`: não testei no processo vivo, para não perturbar a sessão. O número de repaginação vem de um teste sintético de 64 MiB.
- A origem dos ~100 Hz da thread NVIDIA: há relatos de que o ICD Vulkan da NVIDIA acorda a dGPU na inicialização (NVIDIA Developer Forums, driver 550.67, 2024; CachyOS, 610.57). Não há explicação publicada para um timer de 100 Hz.

## Fontes

- Microsoft Learn, *UWP app lifecycle* (13/09/2023): https://learn.microsoft.com/en-us/windows/uwp/launch-resume/app-lifecycle
- Microsoft Learn, *Reduce memory usage when your app moves to the background* (2017, rev. 2022): https://learn.microsoft.com/en-us/windows/uwp/launch-resume/reduce-memory-usage
- Microsoft Learn, *IDXGIDevice3::Trim*: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidevice3-trim
- Petri, *Memory Compression in Windows 10 TH2* (2015): https://petri.com/memory-compression-in-windows-10-threshold-2/
- BleepingComputer (20/04/2019): https://www.bleepingcomputer.com/news/microsoft/windows-10-start-menu-gets-its-own-process-in-build-1903/
- Windows Latest (21/04/2019): https://www.windowslatest.com/2019/04/21/windows-10-may-2019-update-start-menu-improvements/amp/
- TechLasi: https://techlasi.com/savvy/startmenuexperiencehost-exe-guide/
- ElevenForum, SearchHost: https://elevenforum.com/t/stop-searchhost-exe-from-running.35311
- Winaero (rev. 10/03/2026): https://winaero.com/windows-11-start-menu-revealed-as-resource-heavy-react-native-app-sparks-performance-concerns
- PCWorld (27/07/2026): https://www.pcworld.com/article/3199779/microsoft-admits-its-windows-11-apps-hog-ram-a-fix-is-coming.html
- KDE Discuss, Plasma 6.1: https://discuss.kde.org/t/plasma-6-1-is-it-normal-for-plasmashell-process-to-always-use-ram-700mb/17571
- dlauncher (comparação com o Ulauncher): https://rust-digger.code-maven.com/crates/dlauncher
- LWN, MADV_COLD/MADV_PAGEOUT (2019): https://lwn.net/Articles/790123/
- man7 madvise(2) e malloc_trim(3) (man-pages 6.19): https://man7.org/linux/man-pages/man2/madvise.2.html , https://man7.org/linux/man-pages/man3/malloc_trim.3.html
- Kernel, cgroup v2: https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html
- mimalloc, opções de ambiente: https://microsoft.github.io/mimalloc/environment.html
- NVIDIA Developer Forums, Vulkan ICD acorda a dGPU (550.67): https://forums.developer.nvidia.com/t/550-67-nvidia-vulkan-icd-wakes-up-dgpu-on-initialization-and-exit/288095
