# Third-party notices

No code or fixtures from external repositories have been adapted into
`risksieve` as of this writing. This file will record, for each future
adaptation:

- the original file and commit;
- its license;
- the required notice;
- an explanation of the transformation;
- confirmation that the original copyright notice was preserved.

## Reference repositories and their licensing status

For context when that day comes (AGENTS.md section 11):

- **Foundational CRC** reference repository: MIT. Adaptation permitted
  subject to license and attribution requirements.
- **SCoRE** (<https://github.com/Tian-Bai/SCoRE>): MIT. Adaptation
  permitted subject to license and attribution requirements. May also be
  used as a behavioral test oracle.
- **Non-monotonic CRC** (<https://github.com/aangelopoulos/nonmonotonic-crc>):
  README displays an MIT badge, but no root `LICENSE` file was found
  during the initial project review. Treated as all-rights-reserved for
  code-reuse purposes until clarified. Only independently generated
  fixtures may be used from this repository; its source must not be
  copied or translated.
- **Anytime-valid CRC**: no official implementation was identified in the
  paper during the initial review; implemented independently from the
  paper.

Citing a paper and independently implementing its mathematical method is
not the same as reusing its prose, figures, notebooks, or source code; see
AGENTS.md section 11 for the full clean-room policy.
