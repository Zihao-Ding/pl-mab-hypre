import os
import sys


def parse_sat(sat_file):
  d = dict()
  with open(sat_file, 'r') as f:
    for line in f:
      values = list(map(int, line[1:].strip().split()))
      for value in values:
        d[abs(value)] = (value > 0)
        d[-abs(value)] = (value < 0)
  return d

def verify(cnf_file, satd):
  sat = True
  with open(cnf_file, 'r') as f:
    lines = f.readlines()
    for i in range(1, len(lines)):
      satl = False
      values = list(map(int, lines[i].strip().split()))
      for value in values:
        satl = satl or satd[value]
      sat = sat and satl
  return sat

def main():
  sd = parse_sat(sys.argv[2])
  res = verify(sys.argv[1], sd)
  if res:
    with open("verified.txt", 'a+') as f:
      f.write(sys.argv[1] + '\tVERIFIED\n')
    sys.exit(0)
  else:
    with open("verified.txt", 'a+') as f:
      f.write(sys.argv[1] + '\tUNVERIFIED\n')
    sys.exit(1)

if __name__ == "__main__":
    main()
