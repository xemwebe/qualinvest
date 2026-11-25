# gibtax

Standard reports provided by InteractiveBrokers due not provide the required information
by German tax forms. The German Tax report provided by InteractiveBrokes Inc. is not reliably and
timely provided. This Tool should help to extract the required information for German tax forms
from standard activity statements. Since anybody who is required to fill in German tax reports
should be able to understand some German, the rest of this document is in German.

Die Standardberichte und Queries, die InteractiveBrokers bereitstellt, sind ungeeignet zur Erstellung
einer Steuererklärung wie sie deutsche Finanzämter erwarten. InteractiveBrokers stellt zwar großzügigerweise
einen German Tax Report bereit, der aber intransparent und nicht zuverlässig zeitnah bereitgestellt wird.
Dieses kleine Programm wertet Standarreports von InteractiveBrokers aus und generiert die für die deutsche
Steuererklärung notwendingen Informationen.

Vorab aber eine **WARNUNG**: Ich bin selbst keine Steuerexperte und behaupte nicht, dass die von diesem 
Tool zusammengestellten Informationen alle Vorgaben der Finananzämter korrekt erfüllen und von den Finanzämtern
aktzeptiert werden. Insbesondere gibt es etliche Spezial- und Sonderfälle die nicht oder nicht korrekt
abgebildet werden. Diese Dokumention dient vor allem mir selbst, damit ich nächsten Jahr, wenn die nächste
Steuererklärung fällig ist, noch nachvollziehen kann, was ich hier eigentlich gemacht habe. Wer dieses Tool
hilfreich findet und als Hilfsmittel zur Erstellung seiner eigenen Steuererklärung verwedet, darf das gerne
tun, muss aber die volle Verantwortung dafür übernehmen und selbstverständlich überprüfen, dass die Angaben
korrekt sind.

## Voraussetzungen

Um dieses Tool einsetzen zu können, müsen gewisse Einstellungen in InteractiveBrokers vorgenommen werden, damit
die Reports in einem Format erstellen werden, dass von diesem Tool gelesen werden. Zum einen geht das Tool davon
aus, dass Deutsch als Sprache eingestellt wird und das EUR als Basiswährung gewählt wurde. Letzeres ist
möglicherweise nicht erforderlich, ich habe aber damit nicht getestet. Deutsch als Sprache ist jedoch erforderlich,
weil die Kontoauszüge (englisch Activity Statements) in einem CSV mehrere Tabellen ausliefern, deren Header und
Tabellenbezeichnung (erste Spalte) in deutsch sind (zumindest manche).

Folgende Reports werden verwendet:

- Kontoauszug zur Bestimmung der Einstandskurse. Dieser ist optional und für den Fall gedacht, dass bei 
  Eröffnung des Kontos ein Portfolioübertrag von einem anderen Broker stattgefunden hat. Die Einstandskurse 
  und -positionen werden dann aus der Tabelle "Offene Positionen" ausgelesen. 
- Reports mit Tradehistorie (optional, beliebig viele). Wichtig ist hier die Tabelle mit der Tansaktionshistorie,
  das sind die Zeilen mit "Transaction History" in der ersten Spalte. Wie der Report erstellt wurde (über Flex
  Query oder einen andere Abfrage) ist letztlich egal (hoffe ich). Diese Reports dienen dazu, die richtigen
  Einstandskurse zur ermitteln, insbesondere wenn das Portfolio schon länger als ein Jahr existiert. 
- Kontauszug für das jeweilige Jahr, für dass man die Steuererklärung erstellen möchte. Ich erstelle mir immmer
  einen Kontoauszug für das komplette Jahr.
- Wechselkurse. Dazu verwende ich die [EZB Referenzwechselkurse](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html). Hier kann man die Time Series als CSV herunterladen.

## CLI Interface

Ich gehe davon aus, dass der potenzielle Nutzer selbst wissen, wie sie aus dem Source-Code ein lauffähiges
Program erstellen. Das Programm kann dann mit `gibtax --help` gestartet werden, um sich die Hilfe anzuzeigen.

Es werden drei Kommandos unterstützt, die im folgenden erläutert werden.

### Berechne FIFO-Stack
Dieses Kommando dient dazu, die Einstandskurse für Aktien und ETFs under Anwendung von FIFO so zu
ermitteln, wie sie die deutsche Steuerbehörden erwarten. Die Beispiele gehen davon aus, dass die Steuerunterlagen
für 2025 erstellt werden sollen. Ein Aufruf sieht wie folgt aus:

```
gibtax fifo -i <IntialPosition>.csv -m "2025-01-01" -t <Transaktionshistorie1>.csv \
    -t <Transaktionshistorie2>.csv -o fifo_2025.json -f <Referenzwechselkurse>.csv
```
Hier ist 
  - `<InitialPosition>.csv`: Ein Kontoauszug zum ermitteln der Initialen Einstandskurse, die aus den offenen Positionen ausgelesen werden (siehe oben). Dieses Flag is optional
  - Mit dem Flag `-m` gibt man das Datum an bis zu dem (exklusiv dieses Datum selbst) die Einstandskurse für spätere Verkäufe ermittelt werden.
  - Mit dem Flag `-t` können beliebig viele Reports angehängt werden, um z.B. über mehrere Jahre hinweg Käufe und Verkäufe zur Ermittlung der für 2025 gültigen Einstandskurse zu ermitteln.
  - mit dem Flag `-f` werden die tagesgenauen Referenzwechselkurse übergeben.
  - mit dem Flag `-o` kann angegeben werden, wo die Ergebnisse abegespeichert werden sollen, um sie bei der späteren P&L-Ermittlung verwendet zu werden.

### Übersichtsreport erstellen

Mit dem folgenden Kommando kann ein Report erstellt werden, der einne Übersicht über die wesentlichen Gewinne und Verluste erstellt. Der Aufruf erfolgt wie folgt:

```
gibtax report -k <Kontoauszug2025>.csv -F fifo_2024.json -f <Referenzwechselkurse>.csv -o fifo_2025.json
```

Hier gilt
 - `<Kontoauszug2025>.csv` ist der Kontoauszug für das komplette Jahr 2025
 - mit Flag `-F` wird die im mit dem Kommando `fifo` erstellte Datei zur Einstandskursermittlung angegeben
 - mit Flag `-f` übergibt man wie oben die Referenzwechselkurse
 - mit Flag `-o` kann angegeben werden, wo die neue FIFO-Datei abgelegt werden kann, die dann für die Steuererklärung 2026 verwendet werden kann.

Der Erebnisreport enthalt dann Folgendes:

- Die Gesamtsummer realisierte (unversteuerter) erhaltener Zinszahlungen in EUR
- Erhaltene Dividen in EUR nach Währungen getrennt und vor Steuern
- Abgeführte Quellensteuer, nach Jurisdiktionen getrennt. Die abgeführte deutsche Quellensteuer setzt sich aus Kapitalertragssteuer und Soli-Zuschlag zusammen. Ausländische Quellensteuer muss je nach Jurisdiktion anders behandeln werden und müssen nochmal beim Finanzamt versteuert werden. 
- Gewinne und Verluste aus Aktienverkäufe (einschließlich ETFs). Diese Gewinne/Verluste sind unversteuert und werden gemäß FIFO und Umrechnung in EUR am Kauftag (Einstandskurs) bzw. Verkaufstag ermittelt. Hier gibt es kleine Abweichungen zu dem German Tax Report, da für dessen Erstellungen nicht exakt dieselben Wechselkurse verwendeten werden. Da die Erlöse bei IB i.d.R. nicht sofort in EUR umgewandelt werden, ist es ohnehin fraglich, was der "korrekte" Wechselkurs ist.

### Währungsgewinne

Währungsgewinne müssen ähnlich wie Aktiengewinne ermittelt werden. Im German Tax Report werden sie aber nicht explizit
ausgewiesen, jedenfall konnte ich sie nicht finden. Grundlage für die Erstellung ist eine Flex-Query, die eine 
Liste alle Änderungen von Cash-Positionen in Fremdwährung erfasst (EUR-Cash Flows dürfen vorkommen, werden aber ignoriert).
Für die Ermittlung der Einstandspreise wird FIFO wie bei Aktien angewandt, wobei als Preis immer der Wechselkurs am Tag des Cash Flows verwendet wird. Bei expliziten Umtausch von EUR und Fremdwährunge und umgekehrt kommt es dabei zu kleinen Fehlern, da nicht der tatsächlich Umtauschkurs verwendet werden. In vielen Fällen (Zinszahlungen, Dividenden in Fremdwährung, usw.) gibt es aber gar keinen expliziten Umtauschkurs, da nicht wirklich getauscht wird. In diesem Fall ist es nur konsistent, den gleichen Wechselkurs wie für die Berechnung der effektiven EUR-Äquiavalente der Dividenden zu verwenden. Der Aufruf erfolg wie folgt:

```
gibtax curr-report -c <CashBericht_2025>.csv -f <Referenzwechselkure>.csv -o curr_fifo_2025.json \
  -F curr_fifo_2024.json
```

Hier gilt:

- `<CashBericht_2025>.csv` ist das CSV-File mit den Cash-Flows. Es sollte 5 Spalten enthalten ohne Kopfzeile, wobei die erste und fünfte Spalte ignoriert werden. Spalte 2 enthält die Währung, Spalte 3 das Datum im Format "YYYY-MM-DD" und Spalte 4 den Cash-Flow-Betrag.
- Referenzwechselkurse wie oben
- mit dem `-F` Flag kann eine Währungs-FIFO-Datei aus dem Vorjahr, ansonsten wir davon ausgegangen das die Cash-Position zu Beginn des Jahres all gleich 0 sind.
- mit dem `o` Flag kann die aktualisierte Währungs-FIFO-Datei gespeichert werden, um sie im nächsten Jahr wieder zu verwenden.


## Fehlende Features und Bugs

Dieses Tool ist hilfreich für mich, und sei es nur, um mehr Transparenz in den German Tax Report zu erhalten.
Aber dem geneigten Leser wird nicht entgangen sein, dass das eine oder andere Feature fehlt und gewisse
Sonderfälle nicht berücksichtigt sind, wie z.B. Kapitalmaßnahmen. Manches lässt sich durch Tweaken der Input-Files
fixen, anderes nicht. Ich habe eine offenes Ohr für Ergänzungs- und Änderungsvorschläge, aber wenn ich selbst
dafür keine Bedarf habe, werden ich sie nur mit entsprechendem Pull-Request berücksichtigen. 
Steuern machen mir kein Vergnügen, dieses Tool ist eher aus der schieren Not geboren und ich genug interessante
andere Dinge zu tun. Aber als IT-Freelancer bin ich natürlich dafür offen gewisse Spezialwünsche gegen Einwurf
von Münzen zu erfüllen, so weit es meine Auslastung zulässt.

Gleiches gilt für Fehler, die mir berichtigt werden. Sofern ich davon betroffen sind, werde ich sie vermutlich umsetzen, ansonsten ist das nicht sicher. Pull-Request erleichtern das natürlich.
