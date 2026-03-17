---
name: browser-security-audit
description: Audit browser extensions, saved passwords, and update status for security risks
platform: all
last_reviewed: 2026-03-17
author: noah-team
type: system
emoji: 🔍
---

# Browser Security Audit

Checks installed browsers for security issues: suspicious extensions, saved passwords, auto-update status, and tampered default search engines. Focuses on Chrome, Firefox, and Edge.

## When to activate
Routine security review, post-phishing incident, user reports browser behaving strangely, adware/toolbar complaints, or new device onboarding.

## Standard check path

### 1. List installed browser extensions
Check each installed browser for extensions:
- **Chrome**: Read `~/Library/Application Support/Google/Chrome/Default/Extensions/` (macOS) or equivalent. Cross-reference extension IDs with the Chrome Web Store for names.
- **Firefox**: Read `~/Library/Application Support/Firefox/Profiles/*/extensions.json` (macOS) or equivalent.
- **Edge**: Read `~/Library/Application Support/Microsoft Edge/Default/Extensions/` (macOS) or equivalent.

List all extensions with their names and whether they're from the official store.

### 2. Flag suspicious extensions
Flag extensions that match any of these patterns:
- Not from the official browser store (sideloaded).
- Request broad permissions (access to all sites, read/modify all data).
- Unknown or very low user count.
- Known adware/spyware extension names (Hola VPN, various "coupon" or "shopping assistant" extensions, PDF converters with excessive permissions).
- Extensions the user doesn't recognize.

Present findings and let the user decide what to remove.

### 3. Check for saved passwords in browsers
- **Chrome**: Check if `~/Library/Application Support/Google/Chrome/Default/Login Data` exists and has entries (it's a SQLite DB — check file size, don't dump passwords).
- **Firefox**: Check for `logins.json` in the Firefox profile.
- **Edge**: Check the equivalent Edge Login Data file.

If saved passwords are found, warn the user: browser password storage is less secure than a dedicated password manager. Recommend migrating to 1Password, Bitwarden, or similar. Don't be preachy — just note the finding and the recommendation.

### 4. Check browser auto-update status
- **Chrome**: Check `com.google.Keystone` launch agent exists and is loaded (macOS). Chrome self-updates via Keystone.
- **Firefox**: Check Preferences → General → Updates setting (or `prefs.js` for `app.update.enabled`).
- **Edge**: Similar to Chrome, uses a Microsoft update agent.

Flag if auto-updates are disabled. Browsers are a primary attack vector — keeping them current is critical.

### 5. Check default search engine
- **Chrome**: Check `Preferences` JSON file for `default_search_provider_data`.
- **Firefox**: Check `prefs.js` for `browser.urlbar.placeholderName` or search configuration.
- **Edge**: Check Preferences file.

Flag if the default search engine has been changed to something unusual (not Google, Bing, DuckDuckGo, or Yahoo). Adware commonly changes the search engine to ad-supported alternatives.

## Caveats
- Browser profile paths vary by OS. The paths above are macOS — adjust for Windows (`%APPDATA%`) and Linux (`~/.config/`).
- Don't dump or display actual saved passwords — just report that they exist and how many.
- Some enterprise environments manage extensions via policy. Policy-installed extensions are expected and shouldn't be flagged.
- Multiple browser profiles (personal/work) each have their own extensions and settings. Check all profiles.

> Steps 1-5 catch ~85% of browser-level security issues. Most common finding: suspicious or unused extensions.

## Key signals
- **"I keep getting redirected to weird sites"** → search engine hijack (step 5) or malicious extension (step 2).
- **"Toolbars appeared that I didn't install"** → adware extensions. Focus on step 2.
- **"Browser asks me to save passwords"** → step 3, recommend a dedicated password manager.
- **"I got a phishing email and clicked a link"** → check for newly installed extensions and changed settings.

## Escalation
If sideloaded extensions with broad permissions are found and the user didn't install them:
- Document the extension IDs and permissions — do not just remove them, as they may be evidence.
- Recommend a full endpoint security check (activate the `endpoint-security-check` playbook).
- If on a corporate device, notify IT — policy-managed extensions should come from a known source.

## Tools referenced
- Shell commands — read browser config files, check update agents
- Disk audit tools — check browser profile sizes
