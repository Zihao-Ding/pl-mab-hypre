import multiprocessing
import os
from concurrent.futures import ThreadPoolExecutor

dirs=[
    "/data/dataset/SAT/SAT25/",
]

names = []

def instAE(filename):
		os.system('./run0.sh ' + filename)

def run():
    for dir in dirs:
        dir_names = os.listdir(dir)
        for name in dir_names:
            if os.path.isfile(os.path.join(dir, name)):
                names.append(dir + name)
            else: 
                dirs.append(dir + name + '/')

    pool = ThreadPoolExecutor(max_workers=60)
    for name in names:
        pool.submit(instAE, name)
    pool.shutdown()
    
run()
