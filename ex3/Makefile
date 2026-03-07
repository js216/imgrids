CFLAGS = -O3 -I../fonts -I../ex2 -static
EX2_OBJS = ../ex2/shapes.o ../ex2/chars.o ../ex2/mono.o ../ex2/tt.o

demo: demo.o $(EX2_OBJS)
	$(CC) $(CFLAGS) $^ -o $@

$(EX2_OBJS):
	$(MAKE) -C ../ex2

clean:
	rm -f demo
