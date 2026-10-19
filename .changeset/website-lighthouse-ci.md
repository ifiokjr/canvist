---
website: change
---

# Run Lighthouse on mobile too and report scores in CI

The website's CI runs Lighthouse with both the desktop preset and the mobile profile, and writes a median-per-page score table to every run's summary. Desktop must reach 95+ in every category, LCP under 1.5 s and CLS 0. Mobile must reach the same scores and CLS 0, and reports LCP over 1.5 s as a warning. The docs search hint no longer shifts layout.
