# Chirp Pro — Freemium Tier Design Spec

> **Status: DRAFT — Work in progress, not finalized. All pricing, architecture, and feature decisions are subject to change.**

## Context

Chirp is a local-only voice-to-text desktop app. To fund further development, we're exploring a paid "Chirp Pro" tier. The competitive landscape (Wispr Flow, SuperWhisper, Voquill) is crowded — all now support Windows, all offer cloud transcription. **Chirp's moat is privacy.** The free tier processes everything locally with zero network calls. The paid tier adds cloud compute with zero data retention — credible because the free tier proves we mean it.

**Market position:** Privacy-first dictation. The only voice-to-text tool where the free tier never touches a network, and the paid tier retains nothing. Competitors (especially Wispr Flow) have burned trust on privacy — Chirp can own this space.

**Business goals:** Undercut competitors ($5/mo vs $10-15/mo), validate demand before heavy investment, zero fixed costs at zero users.

---

## Competitive Context

| | Chirp Free | Chirp Pro | Wispr Flow | SuperWhisper |
|---|---|---|---|---|
| Price | Free | ~$5/mo | $12-15/mo | ~$8/mo |
| Processing | 100% local | Cloud (zero-retention) | Cloud (audio + screenshots sent to OpenAI/Meta) | Local Whisper + cloud LLMs |
| Privacy | No data leaves device | Audio processed, nothing stored | "Privacy mode" opt-in, trust burned | Local but closed-source |
| Open source | Potential | — | No | No |
| Platforms | Windows + Mac | Windows + Mac | Windows + Mac + mobile | Windows + Mac + iOS |

**Wispr Flow ($81M raised, $700M valuation):** Biggest vulnerability is privacy — caught sending screenshots to cloud, CTO publicly apologized, users actively cancelling.

**SuperWhisper (~$8/mo):** Strong on Mac with local Whisper/Parakeet models. Windows support is weak (no local LLMs yet). Lifetime price jumped from $249 to $849. Closed-source.

**Voquill:** Editor-first writing tool, not system-wide dictation. Different use case.

---

## Tier Structure

### Chirp Free (local-only, forever free)
- Local Parakeet TDT ASR (~500MB model download)
- Local LLM cleanup via llama-server (~1.5GB model download)
- Dictionary & Snippets
- Transcription history (local storage)
- Works fully offline, no account required
- **Zero network calls. Ever.**

### Chirp Pro (~$5/mo, finalize after COGS validation)
- **Cloud ASR** — managed API, no model downloads, instant start, works on weak hardware
- **Cloud LLM cleanup** — better quality using larger models
- **Zero-retention cloud processing** — audio processed, result returned, nothing stored server-side
- **1000 minutes/month** (~33 min/day, covers 99% of users)
- Overage at ~$0.01/min (fair pricing messaging)
- Everything in Free tier still available as fallback
- Falls back to local if offline
- Requires account (email auth)

**Messaging:** "1000 minutes a month — more than you'll ever need. We cap usage to keep Chirp Pro affordable for everyone."

---

## Economics

### COGS per active user (1000 min/mo cap)

| Component | Provider | Cost |
|---|---|---|
| Cloud ASR | AssemblyAI ($0.0025/min) | ~$2.50/mo max |
| Cloud LLM cleanup | Claude Haiku (~$0.0001/call) | ~$0.18/mo |
| Supabase | Free tier | $0 |
| Stripe | 2.9% + $0.30/txn | ~$0.45/mo |
| **Total COGS** | | **~$3.13/mo** |

**At $5/mo → ~37% gross margin.** Average user will use far less than 1000 min, so real margin is higher (~50-60%).

### Migration path (at 500+ users)
Swap managed ASR for self-hosted Parakeet on Modal (serverless GPU, ~$0.33/user/mo at scale) → margin jumps to 80%+. No desktop app changes needed — API contract stays the same.

### Starting infrastructure: $0/mo fixed
- Supabase free tier (auth + DB)
- Stripe (pay per transaction)
- Managed ASR API (pay per minute)
- No servers, no GPUs, no fixed costs

---

## Architecture

```
Desktop App (Tauri)
  ├── Free: audio → local sherpa-onnx → local LLM → inject
  └── Pro:  audio → API server → managed ASR → LLM API → inject
                                  (zero retention)

API Server (Supabase Edge Function)
  ├── POST /transcribe  — auth → subscription check → usage check → ASR API → text
  ├── POST /cleanup     — auth → subscription check → LLM API → cleaned text
  ├── GET  /subscription — tier + usage stats + minutes remaining
  └── POST /stripe-webhook — subscription lifecycle

External Services
  ├── Supabase (auth, profiles table, usage tracking)
  ├── Stripe (billing, checkout, customer portal)
  └── AssemblyAI or Deepgram (ASR — swappable)
```

API server is **stateless**. Auth in Supabase, billing in Stripe, server just validates and proxies. Zero audio/transcript storage.

---

## Desktop App Changes

### Settings/State
Files: `src-tauri/src/state.rs`, `src/stores/appStore.ts`

New fields:
- `tier: 'free' | 'pro'`
- `auth_token: Option<String>` (JWT)
- `user_email: Option<String>`
- `use_cloud_asr: bool` (pro users can toggle to local)
- `use_cloud_cleanup: bool`
- `usage_minutes_remaining: f64`

### New Rust Commands
File: `src-tauri/src/commands.rs`

- `login(email, password) → AuthResponse`
- `signup(email, password) → AuthResponse`
- `logout()`
- `transcribe_cloud(audio_bytes: Vec<u8>) → Result<String>`
- `cleanup_cloud(text: String, dictionary: Vec<String>) → Result<String>`
- `check_subscription() → SubscriptionStatus`
- `open_billing_portal()` (opens Stripe portal in system browser)

### Pipeline Routing
File: `src-tauri/src/commands.rs` — transcription flow

```
if tier == Pro && use_cloud_asr && online {
    transcribe_cloud(audio) → cleanup_cloud(text) or cleanup_local(text)
} else {
    transcribe_local(audio) → cleanup_local(text)  // always works
}
```

### New Frontend UI
Files: `src/` (new components + Settings page additions)

- Account section in Settings: login/signup, subscription status, manage billing
- "Upgrade to Pro" prompts (subtle, not aggressive)
- Cloud/Local toggle for pro users
- Usage meter (minutes remaining this month)
- Privacy badge: "Your audio is never stored" messaging throughout

### Cross-window Sync
Extend existing `settings-changed` event to include auth/tier state. Both overlay and settings windows need tier info to route transcription.

---

## Auth & Billing

### Signup → Subscribe → Use
1. User clicks "Sign In" in Settings → Supabase Auth (magic link or email/password)
2. JWT stored in settings, verified on startup
3. "Subscribe to Pro" → Stripe Checkout in system browser
4. Payment → webhook → Supabase `profiles.tier = 'pro'`
5. Desktop app refreshes subscription status → cloud features unlock

### Cancellation
- Stripe Customer Portal (link from Settings)
- Webhook → tier reverts at period end
- Local functionality unaffected — no punishment for downgrading

### Usage Tracking
- Each `/transcribe` call logs duration to Supabase `usage` table
- At cap: graceful fallback to local with message "Cloud minutes used — using local processing"
- Usage resets monthly

---

## Supabase Schema

```sql
create table public.profiles (
  id uuid references auth.users primary key,
  email text,
  tier text default 'free' check (tier in ('free', 'pro')),
  stripe_customer_id text,
  stripe_subscription_id text,
  subscription_status text default 'inactive',
  current_period_end timestamptz,
  created_at timestamptz default now()
);

create table public.usage (
  id uuid primary key default gen_random_uuid(),
  user_id uuid references public.profiles,
  duration_seconds float,
  endpoint text,
  created_at timestamptz default now()
);

create view public.monthly_usage as
select user_id, sum(duration_seconds) / 60 as minutes_used
from public.usage
where created_at >= date_trunc('month', now())
group by user_id;
```

---

## Privacy Messaging & Branding

**Core promise:** "Your voice stays yours."

**Free tier:** "Everything runs on your device. No accounts, no cloud, no data collection. Period."

**Pro tier:** "Cloud-powered accuracy with zero-retention processing. Your audio is transcribed and immediately discarded — we never store, log, or train on your data."

**vs competitors:** Don't attack by name, but position clearly: "Unlike cloud-first dictation tools, Chirp was built local-first. Our free tier proves we mean it — no network calls, ever. Pro adds cloud compute when you want it, with the same privacy commitment."

---

## Domain & Landing Page

- Acquire chirp.ai
- Landing page: privacy-first messaging, Free vs Pro comparison, email waitlist
- Trust indicators: open-source (if decided), zero-retention policy, no screenshots captured

---

## Implementation Order (when ready to build)

1. Supabase project setup (auth, DB schema)
2. Stripe product + price + checkout + webhook handler
3. Edge functions (/transcribe, /cleanup, /subscription)
4. Rust: auth commands (login, signup, logout, check_subscription)
5. Rust: cloud transcription + cleanup commands
6. Frontend: account UI in Settings, upgrade prompts
7. Frontend: usage meter, cloud/local toggle
8. Pipeline routing: tier-aware transcription path
9. Cross-window sync for auth/tier state
10. End-to-end testing with Stripe test mode

---

## Verification Plan

1. Auth: signup → login → JWT persists across restart
2. Subscribe: Stripe test mode → webhook → tier updates → cloud unlocks
3. Transcribe: record → cloud ASR → result displayed
4. Privacy: verify no audio/transcript stored server-side after processing
5. Usage: transcribe repeatedly → meter decrements → at cap, local fallback
6. Cancel: Stripe portal → webhook → tier reverts → local works
7. Offline: disconnect → pro user falls back to local seamlessly
8. Cross-window: tier change propagates to overlay window
