main.o: main.c config.h \
  utils.h \
  utils.c
config.o: config.c config.h
utils.o: utils.c utils.h
