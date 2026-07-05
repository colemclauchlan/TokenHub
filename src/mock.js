// Demo snapshot used for UI development + headless screenshot verification.
// At runtime the Rust backend replaces this with a real Snapshot over the same shape.
window.MOCK_SNAPSHOT = {
  claude: {
    provider: "claude",
    title: "Claude Code",
    logo: "✳",
    plan: "Max 5×  $100/mo",
    since: "since Feb 1 ›",
    limits: {
      fiveHour: { pct: 36, resetLabel: "3h", source: "providerApi" },
      sevenDay: { pct: 23, resetLabel: "3d", source: "providerApi" },
    },
    hero: {
      tokens: 2_800_000_000, sessions: 6000, messages: 74200, costUsd: 2100,
      breakdown: { cacheRead: 2_600_000_000, cacheWrite: 151_800_000, input: 287_700, output: 1_500_000 },
    },
    today: { msgs: 3451, sessions: 244, tools: 1322, tokens: 151_700_000, costUsd: 129.33,
      breakdown: { cacheRead: 145_400_000, cacheWrite: 5_300_000, input: 14_000, output: 936_700 },
      lastHourRatePerMin: 645_400 },
    // ~60 one-minute samples (tokens/min), spiky
    sparkline: [120,180,90,260,140,320,210,160,300,120,180,430,220,160,120,90,140,260,180,120,
                150,240,180,120,300,220,160,140,120,180,90,120,160,220,180,140,120,260,300,220,
                180,140,120,160,220,300,260,180,140,120,180,240,320,280,220,180,260,340,300,380],
    trend: [
      { day: "Th", msgs: 2600, tokens: 700 }, { day: "Fr", msgs: 700, tokens: 560 },
      { day: "Sa", msgs: 1200, tokens: 600 }, { day: "Su", msgs: 900, tokens: 540 },
      { day: "Mo", msgs: 1700, tokens: 620 }, { day: "Tu", msgs: 1500, tokens: 560 },
      { day: "We", msgs: 800, tokens: 520 }, { day: "Th", msgs: 1300, tokens: 600 },
      { day: "Fr", msgs: 900, tokens: 560 }, { day: "Sa", msgs: 1500, tokens: 640 },
      { day: "Su", msgs: 700, tokens: 520 }, { day: "Mo", msgs: 1200, tokens: 700 },
      { day: "Tu", msgs: 900, tokens: 620 }, { day: "We", msgs: 2100, tokens: 940 },
    ],
    trendPills: { avgPerDay: "1416 msgs/day", totalMsgs: "Σ 19.8K total msgs", totalTokens: "# 782.9K tokens" },
    models: {
      total: 1_700_000,
      list: [
        { name: "opus-4-6", color: "#d0774a", in: 166_400, out: 738_400 },
        { name: "sonnet-4-6", color: "#4aa8c9", in: 73_300, out: 704_500 },
        { name: "opus-4-5 '25", color: "#43b0a3", in: 44_800, out: 16_100 },
      ],
    },
  },

  codex: {
    provider: "codex",
    title: "Codex",
    logo: "◯",
    plan: "Plus  $20/mo",
    since: "since Mar 3 ›",
    limits: {
      fiveHour: { pct: 12, resetLabel: "4h", source: "providerApi" },
      sevenDay: { pct: 41, resetLabel: "5d", source: "providerApi" },
    },
    hero: {
      tokens: 412_000_000, sessions: 820, messages: 9100, costUsd: 190,
      breakdown: { cacheRead: 360_000_000, cacheWrite: 30_000_000, input: 120_000, output: 3_200_000 },
    },
    today: { msgs: 640, sessions: 38, tools: 210, tokens: 22_400_000, costUsd: 18.7,
      breakdown: { cacheRead: 20_100_000, cacheWrite: 1_600_000, input: 9_000, output: 690_000 },
      lastHourRatePerMin: 98_000 },
    sparkline: [40,60,20,80,50,120,70,40,90,30,60,140,60,40,30,20,50,90,60,30,
                40,80,50,30,110,70,40,30,20,60,25,40,55,70,60,45,30,90,110,70,
                60,45,30,55,70,110,90,60,45,30,60,80,120,95,70,55,90,130,110,140],
    trend: [
      { day: "Th", msgs: 400, tokens: 120 }, { day: "Fr", msgs: 220, tokens: 90 },
      { day: "Sa", msgs: 180, tokens: 80 }, { day: "Su", msgs: 260, tokens: 100 },
      { day: "Mo", msgs: 520, tokens: 160 }, { day: "Tu", msgs: 480, tokens: 150 },
      { day: "We", msgs: 300, tokens: 110 }, { day: "Th", msgs: 360, tokens: 130 },
      { day: "Fr", msgs: 240, tokens: 95 }, { day: "Sa", msgs: 200, tokens: 85 },
      { day: "Su", msgs: 180, tokens: 80 }, { day: "Mo", msgs: 500, tokens: 155 },
      { day: "Tu", msgs: 420, tokens: 140 }, { day: "We", msgs: 640, tokens: 210 },
    ],
    trendPills: { avgPerDay: "352 msgs/day", totalMsgs: "Σ 4.9K total msgs", totalTokens: "# 210.4K tokens" },
    models: {
      total: 640_000,
      list: [
        { name: "gpt-5-codex", color: "#57b26a", in: 120_400, out: 402_000 },
        { name: "gpt-5", color: "#4aa8c9", in: 61_300, out: 44_100 },
        { name: "o4-mini", color: "#d9a441", in: 8_800, out: 3_100 },
      ],
    },
  },
};
