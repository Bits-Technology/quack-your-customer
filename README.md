# 🦆 quack-your-customer

> **Agent Pep, Duck Diligence Officer**
> Bits's smallest compliance hire.

Agent Pep is a [Pollen Robotics Microduck](https://pollen-robotics.com/microduck/), a 25 cm open-source biped robot. Pep is trained in simulation and deployed to the office floor, where it performs enhanced duck diligence.

## Responsibilities

- **Incident response:** falls over, then gets back up on its own. Resilience.

## Getting started

Pep has not reported for duty yet, so for now Pep is simulated: the real daemons against a MuJoCo
body. `pep` is the front desk, from inside the repo's Nix shell (`direnv allow` once):

```sh
pep wake             # Pep stands up in a MuJoCo window
pep do roulade       # enhanced duck diligence
pep walk             # a patrol, 8 s
pep status           # still standing?
pep sleep            # end of shift
pep                  # everything else
```

Once the real Pep clocks in:

```sh
ssh microduck
robotctl monitor     # check on Pep
robotctl configure   # adjust Pep's risk appetite
robotctl update      # keep Pep compliant with the latest firmware
```

Behaviours are reinforcement-learning policies trained in MuJoCo. See the upstream [pollen-robotics/microduck](https://github.com/pollen-robotics/microduck) repo for the SDK, simulator and training stack.

## Contributing

PRs more than welcome.

## License

Apache-2.0, same as the upstream Microduck stack.
