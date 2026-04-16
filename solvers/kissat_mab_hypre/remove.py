import os


for filename in os.listdir("./"):
    if filename.endswith(".cnf") or filename.endswith(".sat") or filename.endswith(".out"):
        os.remove(filename)
