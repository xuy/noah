---
name: dns-record-check
description: Check email-related DNS records (MX, SPF, DKIM, DMARC) for a domain
platform: all
last_reviewed: 2026-03-17
author: noah-team
source: bundled
emoji: 📋
---

# DNS Record Check

Looks up email-related DNS records for a domain and reports what's configured, what's missing, and what looks wrong. Useful for diagnosing email delivery problems, setting up a new domain, or verifying DNS changes have propagated.

## When to activate
Email delivery issues (bounces, going to spam), setting up a new email domain, migrating email providers, or verifying DNS configuration after changes.

## Standard check path

### 1. Get the domain
Ask the user for the domain to check using `text_input`. If they give a full email address, extract the domain part (everything after `@`).

### 2. Check MX records
Look up MX records for the domain using a DNS lookup tool or `dig` with type MX.
- Report the mail servers and their priorities.
- Common results: Google (`aspmx.l.google.com`), Microsoft (`*.mail.protection.outlook.com`), Proton (`mail.protonmail.ch`).
- If no MX records exist, email cannot be received at this domain. Flag immediately.

### 3. Check SPF record
Look up TXT records for the domain. Find the one starting with `v=spf1`.
- Report what's included (e.g., `include:_spf.google.com`, `include:spf.protection.outlook.com`).
- Flag if no SPF record exists — this increases the chance of spoofed emails.
- Flag if there are multiple SPF records (only one is allowed per domain — multiple records cause failures).
- Flag common mistakes: missing `~all` or `-all` at the end, too many DNS lookups (limit is 10).

### 4. Check DKIM record
Ask the user for the DKIM selector name using `text_input`. Common selectors:
- Google Workspace: `google`
- Microsoft 365: `selector1`, `selector2`
- Generic: `default`, `mail`, `dkim`

Look up TXT record at `<selector>._domainkey.<domain>`.
- Report if found and valid (should contain `v=DKIM1; k=rsa; p=...`).
- If not found, the selector may be wrong or DKIM isn't configured. Try common selectors automatically before reporting missing.

### 5. Check DMARC record
Look up TXT record at `_dmarc.<domain>`.
- Report the policy: `p=none` (monitoring only), `p=quarantine` (suspicious mail to spam), `p=reject` (block failing mail).
- Flag if no DMARC record exists — the domain has no policy for handling spoofed email.
- Flag if `p=none` with no `rua` tag — monitoring mode with no reporting address means nobody is reviewing the data.

### 6. Summarize findings
Present a clear summary:
- **Configured**: which records exist and look correct.
- **Missing**: which records don't exist and should be added.
- **Issues**: records that exist but have problems (multiple SPF, DMARC with no reporting, etc.).
- **Recommendation**: prioritized list of what to fix first. SPF + DMARC are the most impactful for deliverability.

## Caveats
- DNS changes can take up to 48 hours to propagate, though most propagate within minutes. If records were just changed, re-check later.
- DKIM selectors vary by provider and can be custom. If the user doesn't know the selector, try the common ones listed above.
- Some domains use third-party email services (Mailchimp, SendGrid) that require additional SPF includes and DKIM records. These are separate from the primary email provider's records.

> Steps 1-6 identify ~90% of email DNS misconfigurations. Most common issue: missing or duplicate SPF records.

## Key signals
- **"Emails going to spam"** → likely missing or misconfigured SPF/DKIM/DMARC. Start at step 3.
- **"Can't receive email at this domain"** → missing MX records. Start at step 2.
- **"We just switched email providers"** → check that MX, SPF, and DKIM all point to the new provider.
- **"DMARC reports show failures"** → SPF or DKIM alignment issue. Check both carefully.

## Escalation
If DNS records look correct but email delivery still fails:
- Check with the email provider for account-level blocks or sending limits.
- Use an external tool like MXToolbox or mail-tester.com for a second opinion.
- If the domain is on a blocklist, escalate to the domain administrator for delisting.

## Tools referenced
- DNS lookup tools — DNS lookups for MX, TXT, and other record types
- User input prompts — collect domain name and DKIM selector
