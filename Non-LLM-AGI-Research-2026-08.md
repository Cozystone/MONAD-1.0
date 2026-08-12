# LLM 없이 지능으로 — 비-LLM AGI 아키텍처 전수조사

**조사 시점: 2026년 8월** · 리서치 에이전트 13개(8개 계열 병렬 조사 + 저사양 특화 조사 + 구동 검증 + 지능성 판정 + 완결성 비판·보충)가 웹 교차검증으로 작성. 40여 개 시스템의 GitHub 저장소를 직접 확인함.

**전제 조건**: ① 자기회귀 LLM(다음 토큰 예측기)이 아닐 것 ② 로컬 구동 — 이상적으로는 dGPU 없는 8~16GB RAM 일반 노트북 ③ "진짜 지능" — 세계 모델, 추론, 에이전시(목표 지향 행동), 온라인/지속 학습, 샘플 효율이 실증될 것.

---

## 1. 핵심 결론 (TLDR)

1. **단독 AGI 후보는 아직 없다. 그러나 "AGI에 반드시 이식될 부품 카탈로그"는 완성 단계다.** 요구되는 6가지 지능 속성이 각각 *서로 다른* 시스템에서 실증됐다 — 세계 모델은 Dreamer·JEPA, 지속학습은 NARS·Monty, 추상 추론은 VSA·EBM, 극한 샘플 효율은 EfficientZero·AXIOM. 이들을 **한 시스템에서 동시에** 보인 사례는 없다. 가장 근접한 것이 AXIOM이지만 자사 설계 벤치마크 위에서다.

2. **저사양 노트북(GPU 없음)에서 오늘 당장 "진짜 지능 속성"을 구동할 수 있는 시스템이 실재한다.** 1위 ONA(OpenNARS for Applications): 순수 C, 수십 MB 메모리로 실시간 추론+절차학습, NASA JPL·Cisco 실전 배치 이력. 2위 AXIOM(VERSES): 경사하강 완전 배제, 0.3~1.6M 파라미터, 공식 CPU 지원. 3위 AOgmaNeo: Raspberry Pi Zero에서 60FPS 온라인 학습 실증.

3. **"지능을 싸게 만드는" 소프트웨어 원리는 7가지로 수렴한다** — 국소 학습 규칙(백프롭 제거), 이진/이산 표현, 구조적 사전지식, 프로그램 합성, 비모수 메모리, 재귀(깊이↔시간 교환), 희소 이벤트 구동. 이 원리들이 사용자가 찾는 "아키텍처 자체에서 나오는 효율"의 실체다. (§7)

4. **자본과 실증이 정반대로 움직인다.** 자본은 세계모델·공간지능(World Labs $1B, NVIDIA Cosmos, LeCun의 AMI Labs $1.03B)으로 몰리지만 이들은 오프라인 사전학습 생성 모델이다. 온라인 지속학습·샘플 효율·에이전시를 정면 공략하는 쪽(Sutton의 Oak Lab, Carmack의 Physical Atari, CSCG, AXIOM)은 소형·저자본이지만 **전부 소비자 하드웨어에서 재현 가능**하다. "LLM 스케일링만으로 지속학습 불가"는 가소성 상실 연구(Nature 2024)로 과학적 근거를 얻었다.

5. **결정적 연구 공백 = 기회**: "저사양 로컬에서 온라인 학습하는 세계모델 에이전트"는 아직 아무도 만들지 않았다. 부품은 전부 공개돼 있다. (§10)

---

## 2. 평가 방법

- **지능성 판정**: 6개 축(세계 모델 / 지속학습 / 에이전시 / 추론 / 샘플 효율 / 실증된 범용성) 각 0~10점, 실증된 증거만 인정하고 벤치마크 없는 주장은 대폭 할인. S~C 티어.
- **구동 검증**: 저장소 실존·유지보수 여부를 GitHub에서 직접 확인. `yes-today`(오늘 즉시) / `yes-with-effort`(손이 감) / `partially` / `no` / `vaporware`.
- **저사양 적합성**: dGPU 없는 노트북 CPU 기준 별도 평가.

---

## 3. 종합 순위표

| 시스템 | 지능성 | 저사양 노트북(CPU) | 소비자 GPU 1장 | 핵심 강점 | 치명적 한계 |
|---|---|---|---|---|---|
| **DreamerV3** | **S** | ✗ | ◎ 학습까지 | 150+ 과제 단일 설정, Nature 게재, 독립 재현 다수 | 환경마다 백지 재학습, 전이 0 |
| **V-JEPA 2 (JEPA 계열)** | **S** | ✗ | ○ 추론·파인튜닝 | 비디오만으로 직관물리 학습, 로봇 zero-shot 조작 | 에이전시·지속학습 전무, 전체 청사진은 미완 |
| **AXIOM** | **A** | ○ (파라미터 축소) | ◎ 10분/게임 | 경사하강 없는 온라인 세계모델+계획+호기심, DreamerV3 대비 샘플효율 7.6배 | 자사 설계 벤치마크, 비상업 라이선스 |
| **MuZero/EfficientZero V2** | **A** | ✗ | ◎ (LightZero) | 학습된 모델 위 MCTS = 진짜 계획, Atari 100k 인간급 효율 | 게임당 재학습, 좁고 깊음 |
| **Dreamer 4** | **A** | ✗ | △ 비공식 재현만 | 오프라인 비디오만으로 Minecraft 다이아몬드 | 코드 미공개, 사실상 단일 도메인 |
| **Thousand Brains / Monty** | **A** | ◎ CPU 전용 설계 | 불필요 | 백프롭 없는 지속학습, 14시점으로 98.6% 인식 (동료심사) | 능력이 "3D 물체 인식" 하나뿐 |
| **NARS / ONA** | **B** | ◎ 수 MB, 순수 C | 불필요 | 훈련/추론 구분 없는 실시간 학습, 실전 배치 이력 | 지각은 외부 의존, 스케일 미증명 |
| **HDC/VSA (Torchhd, NVSA)** | **B** | ◎ 수 MB 모델 | 불필요 | RPM 추론 88%, 1-pass 학습, OOD 체계적 일반화 | 에이전트가 아니라 부품 |
| **DreamCoder (프로그램 합성)** | **B** | △ 축소 도메인만 | △ | 유일하게 실증된 "누적 학습"(라이브러리 성장) | 유지보수 뜸, 최전선은 LLM계로 이동 |
| **TRM** | **B** | ✗ (추론만 이론상) | △ 학습은 H100×4 | 7M 파라미터로 ARC-AGI-1 44.6% | transductive — 본 퍼즐 전용, 저장소 아카이브됨 |
| **Genie 3** | **B** | ✗ | ✗ 완전 폐쇄 | 생성적 세계 일관성 독보적 | 지능적 환경이지 에이전트가 아님 |
| **EBM (IRED 계열)** | **B** | ✗ | ○ | 더 어려운 문제로의 OOD 일반화 실증 | 퍼즐 규모, 상용 주장은 검증물 0 |
| **Tsetlin Machine** | B− | ◎ 비트 연산만 | ○ | 해석가능·초저전력 분류, MLPerf Tiny 검증 | 분류기이지 추론기가 아님 |
| **Soar / ACT-R** | C | ◎ | 불필요 | 40년 성숙 코드, 실전 실적 | 지식 수작업 주입, 연구 동력 소진 |
| **OpenCog Hyperon** | C | ○ pre-alpha | 불필요 | 야심찬 자기수정 메타그래프 설계 | 20년간 통합 지능 데모 0건, 마케팅 할인 필수 |
| **HRM** | C | ✗ | ◎ (4070 노트북 명시) | 로컬 친화 저장소 | 독립 검증에서 서사 붕괴 (§8) |
| **Loihi 2 / SpiNNaker 2 (뉴로모픽 칩)** | C | ✗ | 시뮬레이션만 | 5,600배 에너지 절감(실측) | 지능이 아니라 효율의 증거, 개인 접근 불가, Lava 아카이브됨 |
| **Cortical Labs CL1 (배양 뉴런)** | C | ✗ | ✗ $35K 전용 HW | 물리 기판 온라인 학습 유일 실증 | 과제 난이도 Pong 수준 |

*(보충 조사 항목 — Alberta Plan/OaK, DIAMOND, CSCG, GFlowNets, CLRS/Scallop, Hopfield 등 — 은 §6.9~6.13, 전체 40여 개 검증표는 부록 A)*

---

## 4. 저사양 노트북(GPU 없음, 8~16GB RAM)에서 오늘 당장 돌릴 수 있는 Top 5

> 저사양 특화 전담 에이전트가 별도 검증한 순위. Windows에서는 1·3위가 WSL 필요.

### 1위 — ONA (OpenNARS for Applications)
- **무엇**: 비공리적 추론 시스템(NARS)의 실용 구현. 훈련/추론 구분 자체가 없이 매 순간 신념을 수정하는 실시간 추론+절차학습 엔진.
- **왜 1위**: 순수 C·의존성 거의 없음·수십 MB 메모리·ARM/x86, OpenMP 4스레드가 최적점. "제한된 지식·자원 하의 지능(AIKR)"이 설계 철학이라 **8GB 노트북이 오히려 정규 환경**. `./build.sh` 한 번으로 Pong·Cartpole·Space Invaders를 보상 신호만으로 배우는 데모 실행 가능.
- **실증**: NASA JPL first-responder 지원, Cisco 트래픽 감시에 컴포넌트로 실전 배치.
- **한계**: 지각(비전)은 외부 모듈(YOLO 등) 의존, 커뮤니티 수십 명 규모.
- repo: `github.com/opennars/OpenNARS-for-Applications` (MIT)

### 2위 — AXIOM (VERSES)
- **무엇**: 신경망·경사하강·리플레이 버퍼를 전혀 쓰지 않는 객체 중심 베이지안 세계모델 + expected free energy 계획. 픽셀에서 1만 스텝 안에 게임을 온라인 학습.
- **왜 2위**: 파라미터 0.3~1.6M(수 MB), 모델 업데이트 18ms/스텝(A100 기준; DreamerV3는 221ms). **공식 CPU 지원**(계획 롤아웃 수 축소 조건). "이해 기반 지능"에 가장 가까운 실증 — 세계모델·계획·내재적 호기심·지속학습을 동시에 갖춘 유일한 비-LLM 시연.
- **한계**: 벤치마크(Gameworld 10k)가 자사 설계 홈그라운드, 비상업 라이선스(VERSES Academic Research License), CPU 실측치 미공표.
- repo: `github.com/VersesTech/axiom` · arXiv:2505.24784

### 3위 — AOgmaNeo (Sparse Predictive Hierarchies)
- **무엇**: 백프롭 없는 완전 온라인 시퀀스 학습기. 의도적으로 CPU 전용 설계.
- **왜 3위**: **Raspberry Pi Zero에서 60FPS·1천만 시냅스**로 RC카 자율주행(카메라→조향)을 온라인 학습한, 이 목록에서 저사양 하드웨어 실증이 가장 확실한 시스템. 노트북 CPU면 수십 배 큰 모델이 실시간으로 돈다.
- **한계**: 학술 벤치마크보다 데모 중심, 사실상 1인 프로젝트.
- repo: `github.com/ogmacorp/AOgmaNeo`

### 4위 — Tsetlin Machine (tmu)
- **무엇**: 학습·추론 전체가 정수/비트 연산(곱셈 없음)인 명제논리 학습기. 완전한 해석가능성.
- **왜 4위**: CPU(심지어 MCU)가 홈그라운드. MLPerf Tiny 이상탐지에서 신경망 대비 54배 빠른 추론·52배 적은 에너지(독립 벤치마크 궤도). MNIST 99.4%.
- **한계**: 분류기이지 추론기가 아님 — 지능형 시스템의 "초저비용 지각/분류 모듈"로 쓸 것. 입력 이진화 필수, 스케일링 난제.
- repo: `github.com/cair/tmu` (MIT)

### 5위 — pymdp + Torchhd (+ ReservoirPy) "노트북 지능 스택"
- **무엇**: 단일 시스템이 아니라 조합 추천. pymdp(능동추론 에이전트 — 믿음·탐색·EFE 계획), Torchhd(10,000차원 벡터 심볼 연산 — 1-pass 연상 메모리, Raven's Matrices류 추론 재현 가능), ReservoirPy(초 단위로 학습되는 시계열 예측).
- **왜 5위**: 전부 순수 Python/NumPy·pip 설치·CPU 충분. §7의 원리들을 직접 조립·체험할 수 있는 최적 실험대. VSA는 **LLM이 못 하는 OOD 체계적 일반화를 수 MB 모델로 시연**할 수 있는 유일한 선택지(ICML 2025).
- repos: `infer-actively/pymdp` · `hyperdimensional-computing/torchhd` · `reservoirpy/reservoirpy` (모두 MIT)

**차점**: CfC/ncps(19뉴런 주행 제어, Nature MI 게재 — 추론은 MCU급이나 학습이 백프롭), CSCG(CPU 수 분 학습, §6.11), Monty(CPU 전용이나 능력 폭이 좁음), CLRS/Scallop(추론 모듈, §6.12).

---

## 5. 지능성 상위 티어 상세 (6축 점수)

점수: 세계모델 / 지속학습 / 에이전시 / 추론 / 샘플효율 / 실증된 범용성 (0~10)

| 시스템 | WM | CL | AG | RE | SE | GEN | 요지 |
|---|--|--|--|--|--|--|---|
| DreamerV3 (S) | 8 | 4 | 8 | 3 | 6 | 6 | "학습된 세계모델+자율 에이전시"의 가장 넓고 단단한 실증. 인간 데이터 없이 Minecraft 다이아몬드. 단 다이아몬드에 1억 스텝 |
| V-JEPA 2 (S) | 8 | 1 | 3 | 3 | 6 | 5 | 스케일과 실세계 전이를 동시에 보인 유일한 세계모델 노선. 단 현재 실증은 "지각+단기 MPC"뿐 |
| Dreamer 4 (A) | 9 | 1 | 6 | 4 | 7 | 2 | 상호작용 0으로 비디오만 보고 다이아몬드 획득. 깊이는 S급, 폭·검증가능성이 A |
| MuZero/EffZeroV2 (A) | 6 | 2 | 7 | 6 | 9 | 5 | 이 조사에서 가장 진짜에 가까운 "계획으로서의 추론". 좁고 깊은 지능의 정점 |
| AXIOM (A) | 7 | 7 | 6 | 2 | 8 | 2 | 속성 조합은 S급, 증거의 독립성이 A 하단 |
| Monty (A) | 6 | 7 | 4 | 1 | 8 | 2 | 원리의 질은 높으나 입증된 능력이 극히 좁음. 정직성(실세계 66.7% 급락까지 공개)은 최상 |
| NARS/ONA (B) | 3 | 8 | 6 | 5 | 7 | 3 | "항상 켜져 있고 계속 배우는 시스템"을 가장 문자 그대로 시연 |
| HDC/VSA (B) | 1 | 2 | 0 | 7 | 8 | 2 | 비-LLM 진영 추상·조합 추론의 가장 단단한 증거. AGI의 한 조각으로선 S급, 독립 후보로선 B |
| DreamCoder (B) | 2 | 8 | 2 | 7 | 7 | 4 | "누적되는 학습"의 사실상 유일한 구현 |

---

## 6. 계열별 지형도

### 6.1 능동추론 / 자유에너지 원리 (Friston, VERSES)
2025~26년에 "이론뿐"이라는 오명을 처음 실증으로 반박한 진영. AXIOM이 대표(§4-2위). 순수 노선의 미래보다는 신경망 지각과의 하이브리드가 현실적 전망. 3개의 벽: 구조 학습(사전지식을 기계가 만들기), 비정형 지각, 언어. VERSES의 Genius 플랫폼은 클라우드 전용·기업 계약 전용이라 로컬 기준 탈락. pymdp는 교육·프로토타이핑 표준, RxInfer.jl(Julia, 반응형 메시지 패싱)은 스트리밍 베이지안 추론 도구로 훌륭하나 에이전트는 아님.

### 6.2 Thousand Brains / HTM (Hawkins, Numenta)
Monty 프레임워크: 피질 컬럼 + 기준 좌표계(reference frame) + 컬럼 간 투표. **GPU를 아예 쓰지 않는** 헤비안 국소 학습으로 물체당 14시점·98.6% 인식, 파국적 망각이 설계상 없음 — Neural Computation 2026 동료심사 통과. 그러나 입증된 능력은 "시뮬레이션 내 단일 3D 물체 인식·자세 추정" 하나. 계층·조합성·추상·언어는 전부 로드맵. "AGI 후보"가 아니라 "다른 지능 패러다임의 가장 정직하고 검증 가능한 씨앗". 레거시 HTM(NuPIC)은 아카이브 — 신규 진입 이유 없음.

### 6.3 뉴로모픽 / SNN
**"지능의 증거"와 "효율의 증거"가 뚜렷이 분리**된다. 실증 대부분은 효율(Loihi 2 CLP-SNN의 에너지 5,600배 절감 — 실측이나 정확도는 GPU에 뒤짐)이고, 지능 실증은 Spiking-WM(PNAS 2025: 스파이크 세계모델이 GRU와 *동급*이라는 존재 증명)뿐. 결정적 병목은 하드웨어가 아니라 알고리즘 — "뉴런을 늘리면 지능이 는다"는 스케일링 원리가 SNN에는 없다. 주의: **Intel Lava 저장소가 2026-05 아카이브됨**(차세대 SDK 전환), 실칩은 개인 접근 불가. 개인이 만질 수 있는 건강한 생태계는 snnTorch·SpikingJelly(GPU 시뮬레이션)와 $289 BrainChip AKD1000 보드 정도. 폰노이만 CPU에서는 스파이크 희소성의 에너지 이득이 대부분 사라진다는 점이 자주 생략되는 마케팅 포인트.

### 6.4 심볼릭 인지 아키텍처 (Hyperon, NARS, Soar, ACT-R, AERA)
역설의 진영: LLM이 결여한 속성(실시간 연속학습, 극한 샘플 효율, 검사 가능한 추론)을 수십 년 전부터 "실제로" 구현했고 전부 GPU 없이 돌아가지만, **지각(픽셀→심볼)과 지식 규모의 벽**을 못 넘어 좁은 도메인에 갇힘. 실질 활동은 전부 "LLM을 지각·지식층으로, 인지 아키텍처를 제어·추론 골격으로" 쓰는 하이브리드로 수렴 중. 지금 만질 수 있는 유일한 물건은 ONA(§4-1위). Hyperon은 야심 대비 실증이 가장 부족(20년간 통합 지능 데모 0건, 암호화폐 토큰 결합 마케팅 — 대폭 할인 필요). Soar는 Windows 바이너리까지 제공되는 성숙 코드이나 연구 동력 소진.

### 6.5 세계모델 / 모델 기반 RL (JEPA, Dreamer, MuZero, Genie)
**"두 개의 반쪽"이 아직 결합되지 않았다**: 한쪽(DreamerV3·EfficientZero V2)은 상상 속 RL·MCTS 계획이라는 진짜 지능 메커니즘을 소비자 GPU 1장에서 실증했으나 도메인 간 일반성이 0이고, 다른 쪽(V-JEPA 2·Genie 3)은 인터넷 비디오에서 범용 세계 지식을 배우지만 아직 에이전트가 아니다. Dreamer 4가 결합의 첫 실증(코드 미공개). LeCun은 Meta를 떠나 AMI Labs 창업($1.03B 시드, 2026-03) — 자본이 이 경로로 몰리는 신호. 로컬 기준: DreamerV3(MIT, Docker, OOM 대응 문서화까지 — **이 조사에서 가장 안전한 선택지**)와 LightZero(2026-03에도 릴리스, Apache-2.0)는 학습까지, V-JEPA 2는 추론·파인튜닝만, Genie 3는 완전 불가.

### 6.6 초소형 재귀 추론기 / ARC 계열 (HRM, TRM, CompressARC)
이 도메인의 가장 중요한 교훈: **"작은 모델이 추론한다"가 아니라 "점수의 진짜 동인은 테스트 시점 적응 루프"**. HRM의 뇌 영감 계층은 ARC Prize 어블레이션에서 기여 미미로 판명(성능 실체는 정제 루프+test-time training+증강 투표). TRM조차 puzzle-ID 임베딩을 지우면 정확도 0% — 학습 때 본 퍼즐 전용 transductive 시스템. 그럼에도 실용 교훈은 유효: **재귀로 깊이를 시간과 교환하면 파라미터를 100배 줄일 수 있다**(TRM 학습비 $500 미만, CompressARC는 사전학습 0·RTX 4070·퍼즐당 20분). 이 계열의 AGI 경로는 모델 스케일업이 아니라 정제 루프를 DreamCoder식 라이브러리 학습과 결합하는 것 — 정확히 Chollet의 Ndea가 베팅 중인, 아직 아무도 실증 못 한 다음 단계(Ndea는 현재 공개물 0건).

### 6.7 신형 신경 기질 (LNN/CfC, KAN, CTM, Forward-Forward, 예측부호화, NCA)
반복 확인되는 진짜 신호: **추론을 파라미터 조회가 아니라 "시간에 걸친 계산 과정"으로 만드는 기판**(CTM의 내부 동역학, EBM의 에너지 최소화, NCA의 셀 반복)이 수천~수만 파라미터로 OOD 외삽(더 긴 미로, 더 어려운 스도쿠)과 난이도 적응적 계산을 보여준다. 전부 소비자 GPU 1장으로 학습 가능. 그러나 증명된 규모는 2012~2015년 딥러닝 위치(ImageNet 72%, ARC 13%). 스케일을 실제 시도한 유일한 사례(Liquid AI)는 트랜스포머 하이브리드로 회귀 — 순수 기판의 스케일 한계 자인. Forward-Forward는 2026-06 반증 논문이 실데이터 스케일링에 못을 박음. 예측부호화(ngc-learn, PCX)는 5~7층까지는 백프롭 동급이나 ResNet급 깊이에서 붕괴.

### 6.8 와일드카드 (Tsetlin, HDC/VSA, 저수지, 배양 뉴런, 신경진화, Levin)
잔인할 만큼 일관된 패턴: **로컬에서 돌아가고 검증된 것은 좁은 부품 수준 지능**(TM의 해석가능 분류, NVSA의 RPM 87.7%, ESN의 동역학 예측)이고, **AGI 속성이 강한 것은 로컬 불가**(Cortical Labs 배양 뉴런 — $35K 전용 HW, 과제는 Pong 수준). 결정적 신호: open-endedness 창시자들(Stanley, Clune, Sakana)마저 탐색 엔진으로 foundation model을 채택 — 이 도메인에 단독 AGI 후보는 없고 가치는 하이브리드 부품(VSA의 binding, 진화의 탐색 외피, TM의 검증가능 논리).

### 6.9 경험 기반 RL 학파 (Sutton의 Alberta Plan / OaK / Oak Lab, Carmack의 Keen) — 보충 조사
"지능은 데이터셋이 아니라 경험의 스트림에서 나온다." Sutton(2024 튜링상)은 2026-07 **Oak Lab 창업**, LLM 패러다임을 "근본적으로 잘못됐다"고 공개 비판. OaK 아키텍처(런타임 feature 발견 → option·GVF 지식 전환 → 모델 기반 계획)는 아직 완전 구현체가 없는 비전이지만, 부속 성과인 **가소성 상실 연구(Nature 2024)** 는 "표준 딥러닝이 지속학습 환경에서 학습 능력을 잃는다"를 체계적으로 실증 — 이 학파의 핵심 전제에 과학적 근거를 부여했다. 이 학파의 알고리즘(스트리밍 RL, continual backprop)은 리플레이 버퍼 없이 소형 네트워크로 돌게 설계돼 **로컬 친화적**. Carmack의 Keen은 카메라+로봇 조이스틱으로 실제 아타리를 실시간 학습하는 Physical Atari 플랫폼을 2026-06 공개(단일 소비자 GPU, 코드 공개) — 시뮬레이터가 무시하는 실세계 제약(지연, 비정상성)의 표준 테스트베드.

### 6.10 확산 세계모델 / 공간지능 (DIAMOND, Oasis, World Labs, Cosmos, Wayve) — 보충 조사
로컬 관점의 승자는 **DIAMOND**(NeurIPS 2024 spotlight): 확산 모델 세계 안에서 RL 에이전트를 훈련, **RTX 4090 한 장·최대 12GB VRAM·약 2.9일로 전체 학습 재현**, CS:GO 세계모델도 4090에서 ~10fps 플레이 가능 — "소비자 PC에서 돌아가는 세계모델"의 현재 최선. Oasis는 500M 축소판 가중치 공개. World Labs(Fei-Fei Li, Marble — $1B 조달)와 Wayve GAIA는 폐쇄형·클라우드라 로컬 탈락. NVIDIA Cosmos는 2B/7B 오픈 가중치가 4090급 추론 가능하나 LLM 혈통이 짙은 경계 사례.

### 6.11 CSCG / Dileep George 계보 — 보충 조사
해마의 인지 지도 형성을 "앨리어싱된 감각 시퀀스로부터 은닉 그래프 학습"으로 정식화한 구조화 HMM(Nature Communications 2021). 학습 결과가 **사람이 읽을 수 있는 세계의 위상 구조**로 나오고, **CPU에서 수 분~수 시간에 학습**된다. Monty와 같은 신경과학 정공법이되 수리적으로 더 깔끔. 이산 관측·소규모 상태공간 한계. 커뮤니티 JAX/PyTorch 포팅 존재.

### 6.12 신경 알고리즘 추론 / 뉴로심볼릭 (CLRS-30, Scallop, LTN) — 보충 조사
GNN이 고전 알고리즘의 실행 궤적을 배워 절차 자체를 내재화(CLRS-30: 수백만 파라미터, 노트북 GPU로 학습 가능). Scallop(Rust, Datalog 기반 미분 가능 논리)·LTN은 CPU로 충분. LLM이 가장 약한 "체계적 일반화·검증 가능한 정확한 추론"을 정면으로 다루는 추론 모듈 진영. 단독 경로가 아니라 부품.

### 6.13 기타 보충 (GFlowNets/LawZero, Hopfield, 전뇌 에뮬레이션, 열역학 칩)
- **GFlowNets**(Bengio): 보상 비례 확률로 다양한 구조를 샘플링하는 상각 베이지안 추론 — CPU/GPU 1장으로 학습 가능, "이해 축"의 지능. LawZero의 Scientist AI는 비-에이전트 안전 노선(공개 구현 아직 없음).
- **Modern Hopfield / Dense Associative Memory**: attention과의 수학적 등가성(ICLR 2021), 확산-기억 등가성(2025) — 에너지·기억·생성을 하나로 묶는 이론적 접착제. 부품.
- **전뇌 에뮬레이션**: FlyWire 초파리 전뇌(뉴런 13.9만)가 2026년 가상 신체 폐루프 제어까지 도달(운동 예측 ~95%) — 소비자 GPU에서 초파리 스케일 시뮬레이션 가능. "가장 확실하지만 가장 느린 경로".
- **열역학 컴퓨팅**(Extropic TSU, Normal Computing): EBM의 샘플링 병목을 물리로 푸는 기판. Z1(25만 pbit·1W 미만)이 2026년 목표 — 성숙 시(2027~30) 확률 모델 노선 전체의 실행 가능성을 바꿀 "증폭기". 현재는 Fashion-MNIST급 실증뿐. THRML 시뮬레이터는 오픈소스.

---

## 7. "지능을 싸게 만드는" 7가지 소프트웨어 아키텍처 원리

사용자가 요구한 "고효율·고성능·저비용을 아키텍처 자체에서" — 실증된 원리는 이것이다.

| 원리 | 왜 싸지는가 | 실증 대표 |
|---|---|---|
| **국소 학습 규칙** (백프롭 제거) | 전역 backward graph 불필요 → 메모리 급감, GPU 의존 소멸 | AXIOM(변분 베이즈), AOgmaNeo, Monty(헤비안) |
| **이진/이산 표현** | float 행렬곱 → 비트 연산, CPU 캐시 친화 | Tsetlin Machine, HDC/VSA |
| **구조적 사전지식** (객체 중심 등) | 세계의 구조를 아키텍처에 내장 → 파라미터 100~1000배 절감 | AXIOM 1M vs DreamerV3 420M, NCP 19뉴런 주행 |
| **프로그램 합성 / 심볼 압축** | 지식을 파라미터가 아니라 재사용 가능한 코드로 | DreamCoder, ONA, CSCG(읽을 수 있는 그래프) |
| **비모수 메모리** | 학습을 "저장 후 검색"으로 대체 — kNN은 CPU에서 저렴 | Episodic Control, HDC 연상메모리, Hopfield |
| **재귀 — 깊이↔시간 교환** | 작은 망을 여러 번 돌려 깊은 추론 대체 | TRM/HRM/CTM (단 transductive 한계 유의) |
| **희소 / 이벤트 구동 연산** | 바뀐 것만 계산 | AOgmaNeo(k-sparse), SNN(전용 HW에서만 이득) |

핵심 통찰: LLM은 "지식을 파라미터에 저장하고 GPU로 조회"한다. 위 원리들은 지식을 **구조·코드·메모리·시간**에 저장한다 — 그래서 CPU에서 돌 수 있다.

---

## 8. 마케팅 거품 체크리스트 (조사 중 확인된 것)

| 주장 | 실체 |
|---|---|
| HRM "뇌 모방 계층이 추론한다" | ARC Prize 독립 검증: ARC-AGI-1 41%→32%, ARC-AGI-2 2%. 계층 기여 미미, 실체는 정제 루프+TTT |
| TRM "7M으로 LLM을 이긴다" | 절반만 진실 — puzzle-ID 제거 시 0%, 학습은 H100 4장×3일, 저장소 2026-04 아카이브 |
| AXIOM "Google을 이겼다" | 자사 설계 벤치마크 기준. "제3자 검증"은 컨설팅사. 공학적 실증 자체는 진짜 |
| Tsetlin "10,000배 에너지 절감" | 자사 외삽. 독립 검증치는 MLPerf Tiny ~52배 |
| 뉴로모픽 에너지 수치 전반 | 전용 칩 기준. CPU/GPU 시뮬레이션에서는 이득 대부분 소멸 |
| BrainChip "Akida 2 출시" | 2026-08 현재 실리콘 미출하(IP 라이선스만, AKD2500 프로토타입 2026 Q3 예정) |
| VERSES Genius 성과 발표 | 상장사 IR 보도자료 채널 중심, 고객사 익명 |
| Hyperon "AGI 프레임워크" | 실체는 pre-alpha 그래프 DB+패턴 재작성 언어. 20년간 통합 데모 0건 |
| EBM 상용화(Kona 1.0 등) | 공개 벤치마크 0건 — 현재로선 마케팅 |
| "비-LLM Gödel machine" | 23년간 완전 구현 0건. 현대 후계(Darwin Gödel Machine)는 LLM 기반 |

---

## 9. 정직한 평가 — AGI까지의 거리

판정 에이전트의 결론을 요약한다:

- 최상위 후보(Dreamer 계열~V-JEPA 2)조차 AGI와의 거리는 "점진적 개선"이 아니라 **복수의 미해결 통합** 만큼 멀다.
- 전 진영에 공통 결여된 4가지: ① 도메인 간 지식 전이·평생 누적(실증은 장난감 규모 DreamCoder뿐) ② 원시 지각→추상 개념 형성의 다리(추론 실증은 전부 사람이 구조화한 퍼즐 안) ③ 언어·상식(사실상 전 목록에서 0) ④ **"컴퓨트를 부으면 일반성이 늘어난다"는 스케일링 경로** — LLM의 유일한 결정적 자산인데, 어떤 비-LLM 후보도 이를 보여주지 못했다.
- 역설: 가장 강한 비-LLM 결과들(Dreamer 4, V-JEPA 2)은 LLM식 스케일 인프라를 빌려 나왔다.
- 그러나 이들이 실증한 속성 — 온라인 학습, 세계모델 계획, 정보 추구, 극한 샘플 효율 — 은 **현재 LLM에 실제로 결핍된 것들**이다. AGI가 등장한다면 이 계보가 흡수된 하이브리드일 가능성이 높다. 이 목록은 "AGI 후보"가 아니라 **"AGI에 반드시 이식될 부품 카탈로그"** 로 읽는 것이 정직하다.

---

## 10. 전략 제언 — 무엇을 만들 것인가

### 10.1 즉시 실행 (이번 주, 노트북 CPU)
```
1. ONA 빌드 → Pong 절차학습 데모로 "실시간 온라인 학습" 체감   (WSL)
2. pip install inferactively-pymdp → epistemic chaining 튜토리얼로 "믿음 기반 계획" 체감
3. pip install torch-hd → VSA binding/bundling으로 "1-pass 학습·심볼 조합" 체감
```

### 10.2 GPU 1장이 생기면
- DreamerV3(가장 안전) 또는 LightZero로 세계모델 RL 학습 체험
- DIAMOND로 확산 세계모델 학습(12GB VRAM, ~3일)
- CompressARC로 "압축=지능" 테제 직접 실험(퍼즐당 20분)

### 10.3 혁신 기회 — 아직 비어 있는 결정적 공백
**"저사양 로컬에서 온라인 학습하는 세계모델 에이전트"는 아무도 만들지 않았다.** 부품은 전부 공개돼 있다:

```
지각/분류층:  Tsetlin Machine 또는 소형 CNN (초저비용, 해석가능)
세계모델:     AXIOM식 객체 중심 베이지안 혼합모델 (경사하강 불필요)
              또는 CSCG (읽을 수 있는 인지 지도, CPU 수 분 학습)
기억:         HDC/VSA 연상 메모리 (1-pass, 수 MB) + 에피소딕 kNN
추론/제어:    NARS식 자원 제한 추론 (AIKR) 또는 EFE 계획 (pymdp 계보)
지속학습:     continual backprop (가소성 유지, Nature 2024)
```

이 조합은 §7의 7원리를 전부 만족하고, 각 부품이 MIT/Apache 라이선스로 공개돼 있으며(AXIOM만 비상업 — 아이디어는 특허가 아니라 논문), 전부 CPU에서 돈다. 학계·산업 모두 "자본은 대형 세계모델로, 실증은 소형 온라인 학습으로" 갈라진 지금이 이 공백을 파고들 적기다.

### 10.4 관전 포인트 (2026 하반기~2027)
- Sutton의 Oak Lab 첫 산출물 (OaK 구현체가 나오는가)
- Ndea의 첫 공개물 (프로그램 합성+정제 루프의 일반화)
- VERSES AXIOM의 구조 학습 후속 (arXiv:2511.02091 노선)
- Monty의 조합성·계층 로드맵 진척
- Extropic Z1 실리콘 (확률 모델 노선의 게임 체인저 여부)
- Dreamer 4 코드 공개 여부

---

## 부록 A — 전체 구동 가능성 검증표 (GitHub 직접 확인 기준)

| 시스템 | 로컬 구동 | 한 줄 평 |
|---|---|---|
| AXIOM | ✅ 오늘 | GPU 10분/게임, CPU 가능. 비상업 라이선스 |
| pymdp | ✅ 오늘 | 노트북 CPU 충분, JAX 전환 완료, 활발 |
| RxInfer.jl | ✅ 오늘 | Julia 한 줄 설치, 추론 도구이지 에이전트 아님 |
| Monty (Thousand Brains) | ✅ 오늘 | CPU 전용, Windows는 WSL 필수 |
| htm.core (레거시) | ⚠️ 손이 감 | 정체된 레거시, 신규 채택 이유 없음 |
| ONA (NARS) | ✅ 오늘 | 수 MB C 프로그램, WSL/MSYS 필요 |
| Soar | ✅ 오늘 | Windows 바이너리 제공, 동력 소진 |
| ACT-R | ✅ 오늘 | 인지심리 모델링 도구 — AGI 후보 아님 |
| AERA | ⚠️ 손이 감 | VS 솔루션 제공되나 문서 빈약, 스타 20개 |
| Hyperon/MeTTa | ✅ 오늘 | pip 되지만 pre-alpha 인터프리터 |
| DreamerV3 | ✅ 오늘 | 4090 학습까지, MIT, 이 조사 최고 안전 선택 |
| Dreamer 4 | ⚠️ 부분 | 코드 미공개, 비공식 재현뿐 |
| LightZero (MuZero/EffZero) | ✅ 오늘 | 2026-03 릴리스, 4090 수 시간~하루 |
| V-JEPA 2 | ✅ 오늘 | 가중치 공개·HF 통합, 추론·파인튜닝만 |
| Genie 3 | ❌ | 완전 폐쇄, TPU 전용 |
| DIAMOND | ✅ 오늘 | 4090 단독·12GB로 전체 학습 재현 |
| Oasis | ✅ 오늘 | 500M 축소판 가중치 공개 |
| TRM | ⚠️ 주의 | MIT지만 저장소 아카이브, transductive |
| HRM | ✅ 오늘 | 4070 노트북 명시 지원, 서사는 걸러서 |
| CompressARC | ✅ 오늘 | 사전학습 0, 퍼즐당 20분 |
| DreamCoder | ⚠️ 손이 감 | 2021년산, Stitch(Rust)만 가벼움 |
| Ndea | ❌ | 공개물 0건 — 자금 조달된 가설 |
| ncps (LNN/CfC) | ✅ 오늘 | pip 한 줄, CPU 학습 가능 |
| pykan (KAN) | ✅ 오늘 | 튜토리얼 CPU 10분, 니치 도구 |
| CTM (Sakana) | ✅ 오늘 | 체크포인트 공개, 원리 시연 수준 |
| Forward-Forward | ✅ 오늘 | 토이 재현만, 스케일링 반증됨 |
| ngc-learn (예측부호화) | ✅ 오늘 | 활발 유지, 대규모 실증 공백 |
| IRED (EBM) | ⚠️ 손이 감 | 연구 코드, 4090 재현 가능 |
| NCA | ✅ 오늘 | 브라우저에서도 돎, ARC 13.4% 상한 |
| tmu (Tsetlin) | ✅ 오늘 | pip+CUDA, 2026 유지보수 계획 명시 |
| Torchhd (HDC/VSA) | ✅ 오늘 | pip, 수 MB 모델, 활발 |
| ReservoirPy | ✅ 오늘 | CPU 수 초 학습, Inria 유지 |
| snnTorch / SpikingJelly | ✅ 오늘 | 가장 건강한 SNN 생태계 |
| Lava (Loihi 2) | ⚠️ 부분 | **2026-05 아카이브** — 막다른 길 |
| SpiNNaker 2 | ⚠️ 부분 | 기관 판매 전용, e-prop 재현 우회로만 |
| Nengo/Spaun | ✅ 오늘 | 성숙하나 핵심 성과는 10년 전 |
| BrainChip Akida | ⚠️ 부분 | $289 보드 실구매 가능, 소형 CNN 추론용 |
| Darwin Monkey | ❌ | 국가 연구 인프라, 완전 폐쇄 |
| Cortical Labs CL1 | ❌ | $35K 전용 HW 또는 클라우드 구독 |
| FinalSpark | ❌ | 원격 클라우드 접근만 (월 $500) |
| CSCG | ✅ 오늘 | CPU 수 분 학습, 커뮤니티 포팅 존재 |
| CLRS-30 / Scallop / LTN | ✅ 오늘 | 노트북급, 추론 모듈 |
| GFlowNets | ✅ 오늘 | CPU/GPU 1장, 공개 코드 풍부 |
| loss-of-plasticity (continual backprop) | ✅ 오늘 | Nature 2024 공식 코드 |
| Physical Atari (Keen) | ✅ 오늘 | 플랫폼·코드 공개, 단일 소비자 GPU |
| hopfield-layers / HAMUX | ✅ 오늘 | 부품 라이브러리 |
| FlyWire 초파리 에뮬레이션 | ⚠️ 부분 | 곤충 스케일은 소비자 HW 가능 |
| Extropic THRML | ✅ 오늘 | 시뮬레이터만 (실칩 미출하) |
| evosax/EvoJAX (신경진화) | ✅ 오늘 | 중규모 진화 실험 가능 |
| Gödel machine (비-LLM) | ❌ | 구현 자체가 존재하지 않음 |

## 부록 B — 핵심 출처 (선별)

- AXIOM: arXiv:2505.24784 · github.com/VersesTech/axiom · 구조학습 후속 arXiv:2511.02091
- Monty: Neural Computation 2026 · github.com/thousandbrainsproject/tbp.monty
- DreamerV3: Nature 2025(Hafner et al.) · github.com/danijar/dreamerv3 · Dreamer 4: arXiv:2509.24527
- V-JEPA 2: github.com/facebookresearch/vjepa2 · LeJEPA(2025)
- HRM 독립 검증: arcprize.org/blog/hrm-analysis · TRM: github.com/SamsungSAILMontreal/TinyRecursiveModels · TRM 재분석: arXiv:2512.11847
- 가소성 상실: Dohare et al., Nature 2024 · github.com/shibhansh/loss-of-plasticity
- Oak Lab 출범(2026-07) · Silver & Sutton "Welcome to the Era of Experience"(2025-04) · Alberta Plan: arXiv:2208.11173
- Physical Atari: arXiv:2606.19357 · github.com/Keen-Technologies/physical_atari
- DIAMOND: NeurIPS 2024 · github.com/eloialonso/diamond
- CSCG: Nature Communications 2021 · github.com/vicariousinc/naturecomm_cscg
- NVSA(RPM 추론): Nature MI 2023 · ICML 2025 OOD 일반화 · Torchhd: JMLR
- ONA: github.com/opennars/OpenNARS-for-Applications · AGI-20 논문
- Tsetlin 360° 리뷰(2025) · Literal Labs MLPerf Tiny · CLP-SNN(Loihi 2, 2025-11) · Spiking-WM: PNAS 2025
- Extropic: extropic.ai(Z1 로드맵, DoC LOI 2026-07) · Normal Computing CN101(2025-08)
- FlyWire: Nature 2024-10 · 폐루프 에뮬레이션 2026-04 · State of Brain Emulation Report: arXiv:2510.15745

---

*본 리포트는 Claude Code 멀티에이전트 워크플로(리서치 8 + 저사양 특화 1 + 검증 1 + 판정 1 + 비판 1 + 보충 1)로 작성되었으며, 모든 저장소 상태·벤치마크 수치는 2026년 8월 기준 웹에서 교차 확인한 것이다. 성능 수치는 각 출처의 보고값이며, "자사 발표"로 표시된 것은 독립 검증이 없다.*
