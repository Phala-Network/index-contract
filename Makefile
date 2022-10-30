.PHONY: all init init-deps

all:

init: init-deps
	pre-commit install

init-deps:
	pip3 install pre-commit
