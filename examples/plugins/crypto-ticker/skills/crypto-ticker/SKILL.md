---
name: crypto-ticker
description: Show live crypto prices or a coin's price history. Use when the user asks about bitcoin, ether or another coin's price, a crypto chart, or how a coin moved over days.
---

# Crypto ticker

1. Live prices: `tools.crypto_ticker.show_ticker` from run_typescript with
   `coins` (ids or symbols: bitcoin, ethereum, solana, cardano, dogecoin,
   ripple, polkadot, litecoin, chainlink, tether) and `vs` (usd, eur,
   gbp, jpy, chf). The view ticks by itself every 30 s.
2. One price in your answer: `tools.crypto_ticker.get_price` (`coin`, `vs`).
3. History: `tools.crypto_ticker.backfill_history` (`coin`, `days` 7 to
   90). It is slow on purpose (about 2 s per week): say so; the user can
   stop it.
4. If a text says SAMPLE, the data is not real (CoinGecko unreachable or
   rate-limited): say it. Never give financial advice.
