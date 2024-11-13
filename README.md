# ta-agi-doorbell
This Service Exposes AGI scripts.
When called and authenticated via SHA1-Digest, a digital Value ON is sent to defined CMIs by [TA](www.ta.co.at).
The main usecase is sending signals to open doors (connected to TA-controllers) via DTMF signals from SIP-Doorbells connected to asterisk.

# Getting started
TODO
- copy AGI stuff from asterconf
- copy TA stuff from bma-alarm

# Scope
This project is a very specific use of two more general libraries: [coe-rs](https://github.com/curatorsigma/coe-rs) for the integration with TA and [blazing_agi](https://github.com/curatorsigma/blazing_agi) for the integration with AGI.
If you want to build a similar integration for your building, you may want to take this repository as a starting point and develop your own solution instead.

# License
This project is licensed under MIT-0 (MIT No Attribution).
By contributing to this repositry, you agree that your code will be licensed as MIT-0.

For my rationale for using MIT-0 instead of another more common license, please see
https://copy.church/objections/attribution/#why-not-require-attribution .

