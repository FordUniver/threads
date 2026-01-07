PERL = perl
PROVE = prove -l
PERLCRITIC = perlcritic

.PHONY: test install lint clean deps

deps:
	cpanm --installdeps .

test:
	$(PROVE) t/

install:
	install -d $(HOME)/.local/bin
	install -m 755 bin/threads $(HOME)/.local/bin/threads-perl

lint:
	$(PERLCRITIC) --stern lib/ bin/

clean:
	rm -rf blib _build cover_db

dev-link:
	ln -sf $(PWD)/bin/threads $(HOME)/.local/bin/threads-perl
