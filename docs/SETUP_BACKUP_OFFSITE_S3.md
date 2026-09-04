# Como configurar o backup offsite (S3) em cada PC

Guia pratico pra ativar o upload automatico do backup diario pra AWS S3 — feito uma vez
por conta AWS (bucket + policy + usuario), e depois uma vez por PC (arquivo de
configuracao). O app ja tem o codigo pronto (`db::backup_nuvem`, ver
`docs/ARQUITETURA.md`); isso aqui e so a parte manual, feita fora do app.

Sem esse arquivo configurado, nada muda — o backup local e o externo (pendrive/HD)
continuam funcionando normalmente, so o upload pra nuvem fica pulado.

## Por que

Hoje o backup local (14 dias) e o externo (pendrive/HD) protegem contra falha de disco,
mas o pendrive normalmente fica no mesmo local fisico que o PC — nao protege contra
incendio, roubo ou qualquer coisa que atinja o site inteiro. O S3 e a copia que fica
fora desse risco.

## Passo 1 — Criar o bucket S3

1. Entrar no [console da AWS](https://console.aws.amazon.com/s3/) → **S3** → **Create
   bucket**.
2. Nome do bucket: algo unico globalmente, ex. `ecoviva-backups` (se ja existir, tenta
   `ecoviva-backups-2026` ou parecido).
3. Regiao: `sa-east-1` (São Paulo) — mais perto, menor latencia.
4. **Block all public access**: deixar marcado (padrao) — o bucket nunca deve ser
   publico.
5. Deixar o resto no padrao e criar.

## Passo 2 — Criar a IAM policy (permissao minima: so enviar, nunca apagar)

Essa e a parte mais importante: a credencial que vai ficar gravada no PC **so pode**
enviar arquivo pro bucket, nunca apagar. Isso e proposital — se o PC for comprometido
(virus, ransomware, erro humano), quem tiver essa credencial nao consegue destruir os
backups que ja foram enviados.

1. Console da AWS → **IAM** → **Policies** → **Create policy** → aba **JSON**.
2. Colar (trocando `ecoviva-backups` pelo nome real do bucket criado no Passo 1):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "SoEnviarNuncaApagar",
      "Effect": "Allow",
      "Action": "s3:PutObject",
      "Resource": "arn:aws:s3:::ecoviva-backups/*"
    }
  ]
}
```

3. Nome da policy: `ecoviva-backup-put-only`.
4. Criar.

## Passo 3 — Criar o usuario (IAM user) e gerar a chave de acesso

1. Console da AWS → **IAM** → **Users** → **Create user**.
2. Nome: `ecoviva-backup-nuvem` (nao precisa de acesso ao console, so a chave de API).
3. Anexar a policy `ecoviva-backup-put-only` criada no Passo 2.
4. Depois de criado, entrar no usuario → aba **Security credentials** → **Create access
   key** → tipo "Application running outside AWS" (ou "Other").
5. Anotar os dois valores mostrados: **Access key ID** e **Secret access key** — o
   secret so aparece uma vez, se perder tem que gerar outro.

## Passo 4 — Criar `backup_nuvem.txt` em cada PC

Na pasta de dados do app em cada PC (a mesma onde ja fica `turso.txt` — no Windows,
normalmente `%APPDATA%\com.ecoviva.controlearmazem\`), criar um arquivo de texto
chamado `backup_nuvem.txt` com 5 linhas:

```text
AKIA...                  (access key id gerado no Passo 3)
wJalrXUtn...              (secret access key gerado no Passo 3)
ecoviva-backups           (nome do bucket criado no Passo 1)
sa-east-1                 (regiao do bucket)
A4                        (prefixo - ver observacao abaixo)
```

**A ultima linha (prefixo) tem que ser diferente em cada PC** — ex. `A4` no PC do
armazem A4, `B2` no do armazem B2. Os dois PCs enviam pro mesmo bucket, e o prefixo e o
que evita um sobrescrever o backup do outro.

## Passo 5 — Confirmar que funcionou

1. Abrir o app normalmente (o upload roda uma vez por abertura, em segundo plano, sem
   travar nada).
2. Esperar um minuto e conferir no console da AWS: **S3** → o bucket → deve aparecer
   uma pasta com o prefixo configurado (`A4/` ou `B2/`) contendo o `.db` do dia, o dump
   do Turso (se `turso.txt` tambem estiver configurado) e as copias de `turso.txt`/
   `backup_externo.txt`.
3. Se nao aparecer nada, olhar o log do app (mesmo lugar onde ja se olha pra erro de
   sincronizacao do Turso) — falha aqui nunca trava o app, so fica registrada como
   aviso.

## Custo esperado

O banco desse app e pequeno (poucos MB no maximo) e o upload e diario — o custo de
armazenamento e de `PutObject` no S3 pra esse volume fica na casa de centavos por mes,
mesmo fora do free tier. Nao ha necessidade de configurar Lifecycle/expiracao a menos
que o usuario queira limitar o historico guardado — por padrao os objetos ficam no
bucket indefinidamente (o app nunca apaga nada de la, de proposito — ver Passo 2).
