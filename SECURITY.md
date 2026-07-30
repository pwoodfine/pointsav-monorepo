# PointSav and Woodfine Security Policy

Version 1.0 — Effective 2026-04-20
Copyright (c) 2026 Woodfine Capital Projects Inc. All rights reserved.

## Preamble

This Security Policy ("this Policy") describes how Woodfine Capital
Projects Inc., PointSav Digital Systems, and Woodfine Management Corp.
(collectively, "Woodfine") receive, triage, and disclose security
vulnerabilities in Woodfine-managed software and services. It is
incorporated by reference into the PointSav Commercial License
(Section 6). Commercial customers may have additional channels defined
in their Order Form or Support Addendum; this Policy is the primary
and authoritative channel for unsolicited disclosures from the public.

---

## 1. Scope

This Policy applies to all repositories managed through the
factory-release-engineering directory, including those in the PointSav
Digital Systems GitHub organization and the Woodfine Management Corp.
GitHub organization, regardless of the license applied to the individual
repository.

In scope:

  (a) Vulnerabilities in source code published by Woodfine under any
      license issued from factory-release-engineering.
  (b) Vulnerabilities in build artifacts, container images, or
      packages distributed by Woodfine.
  (c) Vulnerabilities in Woodfine-operated services and
      infrastructure that are directly reachable by third parties.

Out of scope (see Section 7 for expanded list):

  (a) Third-party dependencies where Woodfine is not the maintainer
      — report those upstream.
  (b) Non-security bugs. Report those via normal issue trackers.
  (c) Issues arising from use outside the terms of the applicable
      license.

---

## 2. Reporting a Vulnerability

Primary channel: email to open.source@pointsav.com.

Alternative channel: the GitHub Security Advisories "private
vulnerability reporting" feature on the affected repository, which
provides an encrypted submission channel visible only to Woodfine
maintainers.

Please include in Your report:

  (a) The affected repository, component, or service.
  (b) The version or commit hash where the vulnerability was
      observed.
  (c) A description of the vulnerability and its potential impact.
  (d) Step-by-step reproduction instructions.
  (e) Any proof-of-concept code or screenshots (sensitive material
      may be encrypted — see Section 2.1).
  (f) Your contact information and preferred disclosure timeline.
  (g) Whether You wish to be credited publicly and, if so, how.

### 2.1 Encrypted Submission

Woodfine does not publish a PGP key at this time. Reporters submitting
sensitive material should use the GitHub Security Advisory channel on
the affected repository, which provides transport encryption and
access control by default. Reporters who require an alternative secure
channel may contact open.source@pointsav.com to arrange one.

---

## 3. Response Commitments

Woodfine commits to the following timelines for reports submitted
through the channels in Section 2:

  (a) Acknowledgment of receipt: within five (5) business days.
  (b) Triage and initial severity assessment: within thirty (30)
      calendar days of acknowledgment. Woodfine will share a
      preliminary remediation plan at this stage.
  (c) Remediation: best-effort, prioritized by severity. Woodfine
      does not commit to a specific fix timeline in this version of
      the Policy.

These commitments are appropriate for Woodfine's current operational
posture. When Woodfine establishes a dedicated security function,
this Policy will be amended to reflect tighter service-level
commitments.

---

## 4. Coordinated Disclosure

Woodfine follows a coordinated disclosure model:

  (a) Public disclosure is withheld while Woodfine develops and
      distributes a fix.
  (b) The default disclosure window is ninety (90) calendar days
      from the date of acknowledgment (Section 3(a)). This window
      aligns with standard industry practice, including the
      disclosure timelines of Google Project Zero and the CERT
      Coordination Center.
  (c) Extensions to the disclosure window may be agreed between
      Woodfine and the reporter in writing where a fix requires
      additional time.
  (d) Earlier disclosure is permitted, and may be required, if the
      vulnerability is being actively exploited or has already been
      publicly disclosed by a third party.

---

## 5. Safe Harbor

Woodfine will not pursue legal action or request law-enforcement
investigation against researchers who, in good faith:

  (a) Report vulnerabilities through the channels in Section 2.
  (b) Access, store, modify, or delete only the minimum data
      necessary to demonstrate the vulnerability.
  (c) Avoid interrupting Woodfine services or degrading the
      experience of other users.
  (d) Provide Woodfine a reasonable opportunity to fix the
      vulnerability before public disclosure (see Section 4).
  (e) Comply with applicable law.

This safe harbor language is consistent with guidance from the U.S.
Department of Justice (May 2022) and Public Safety Canada, which
distinguish good-faith security research from unauthorized computer
access.

Researchers uncertain whether a planned investigation falls within
this safe harbor are encouraged to contact open.source@pointsav.com
in advance.

---

## 6. Credit and Recognition

Woodfine acknowledges the contribution of security researchers who
report vulnerabilities responsibly. By default, reporters are
credited by name in the public advisory when the fix is disclosed.
Reporters may elect anonymity, in which case Woodfine will describe
the report as "reported anonymously."

Woodfine does not at this time maintain a separate "hall of fame"
page; credit is provided in individual advisories only. A consolidated
recognition page may be added as the program matures.

---

## 7. Out of Scope

The following are outside the scope of this Policy and should not be
reported through the channels in Section 2:

  (a) Social engineering of Woodfine employees, contractors,
      partners, or users.
  (b) Physical security issues at Woodfine premises.
  (c) Denial-of-service attacks or vulnerabilities whose only
      demonstration requires a denial of service.
  (d) Vulnerabilities requiring privileged network access already
      obtained through unrelated means.
  (e) Vulnerabilities in third-party dependencies that Woodfine does
      not maintain. Report these upstream.
  (f) Non-security bugs (report through the normal issue tracker of
      the affected repository).
  (g) Issues arising from use of Woodfine software outside the scope
      of its applicable license.

Good-faith testing of in-scope targets that inadvertently touches
out-of-scope material will be handled under Section 5.

---

## 8. Bug Bounty

Woodfine does not currently operate a formal bug bounty program. No
monetary rewards are offered for vulnerability reports at this time.
This Policy may be amended to establish a bounty program in the
future.

---

## 9. CVE Assignment

For validated vulnerabilities with material impact, Woodfine will
request a Common Vulnerabilities and Exposures (CVE) identifier
through MITRE or through the GitHub Security Advisory Database,
which is a CVE Numbering Authority (CNA). Advisories published by
Woodfine will reference the CVE identifier where one has been
assigned.

---

## 10. Commercial Customers

Customers of Woodfine under the PointSav Commercial License may
have additional security channels, SLAs, or notification
requirements defined in their Order Form or a Support Addendum.
Those commercial channels supplement — and do not replace — this
Policy. Commercial customers are encouraged to use their contracted
channel for privileged disclosures while this Policy remains the
authoritative public channel.

---

## 11. Amendment

Woodfine may amend this Policy at any time. The version and date at
the top of this document identify the operative text. Amendments
take effect upon publication in the factory-release-engineering
directory.

---

## 12. Governing Law

This Policy and any dispute arising out of or related to it shall
be governed by the laws of the Province of Ontario and the federal
laws of Canada applicable therein. This choice of law does not
constrain the criminal law of the jurisdiction in which a
researcher operates.
