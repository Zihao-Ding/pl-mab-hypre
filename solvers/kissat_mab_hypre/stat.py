import os


sat = unsat = par2 = 0
with open("result.txt", "r") as f:
  for line in f:
    kd = line.strip().split('\t')
    name = kd[0]
    res = int(kd[1])
    if len(kd) == 2:
      kd.append('0')
    time = float(kd[2]) if res != 0 else 10000

    if res == 1:
      sat += 1
    if res == -1:
      unsat += 1
    par2 += time

print(sat, unsat, sat + unsat, par2 / 400)
