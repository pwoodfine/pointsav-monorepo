---
title: "ToteboxOS"
slug: os-totebox
category: architecture
subcategory: foundation-layer
last_edited: 2026-04-22
editor: pointsav-engineering
status: stable
references:
  - id: 1
    text: "Klein, G. et al. seL4: Formal Verification of an OS Kernel. ACM SOSP 2009."
    url: "https://sel4.systems/"
---
ToteboxOS (`os-totebox`) is a self-contained archive for one specific legal asset —
a building, a company, or a person. All data is stored as flat files. Software
engines read and process those files, but the files exist independently of the
engines.[^1]

## Archive Types

Three archive types are in use, each anchored to a universal legal identifier.

### PropertyArchive

Anchored to a land title PIN. Holds permits, BIM drawings, lease register,
IoT data, and maintenance history.

### CorporateArchive

Anchored to a business incorporation number. Holds financial records, minute
books, and statutory ledgers.

### PersonnelArchive

Anchored to a SIN or passport ID. Holds identity records and professional
network data.
