# Requires PowerShell 7 and network access. The build uses the downloaded files offline.
$ErrorActionPreference = 'Stop'
$inputFolder = Join-Path (Split-Path $PSScriptRoot) 'inputs'
$null = New-Item -ItemType Directory -Force -Path $inputFolder
$sources = [ordered]@{
    'wiki_collocation_data.txt.zip' = 'https://raw.githubusercontent.com/iris2hu/Chinese-collocation-complexity/main/collocation_data/wiki_collocation_data.txt.zip'
    'edu_collocation_data.xlsx' = 'https://raw.githubusercontent.com/iris2hu/Chinese-collocation-complexity/main/collocation_data/edu_collocation_data.xlsx'
    'twngrams.tsv' = 'https://huggingface.co/datasets/taiwan-corpora/twngrams/resolve/2931fe6/token_2gram.tsv'
    'cedict.txt.gz' = 'https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.txt.gz'
    'ramc-train.parquet' = 'https://huggingface.co/api/datasets/EaseZh/magicdata_ramc/parquet/default/train/0.parquet'
    'ramc-validation.parquet' = 'https://huggingface.co/api/datasets/EaseZh/magicdata_ramc/parquet/default/validation/0.parquet'
    'ramc-test.parquet' = 'https://huggingface.co/api/datasets/EaseZh/magicdata_ramc/parquet/default/test/0.parquet'
    'dictionary-index-schema.json' = 'https://raw.githubusercontent.com/yomidevs/yomitan/master/ext/data/schemas/dictionary-index-schema.json'
    'dictionary-term-bank-v3-schema.json' = 'https://raw.githubusercontent.com/yomidevs/yomitan/master/ext/data/schemas/dictionary-term-bank-v3-schema.json'
}

foreach ($source in $sources.GetEnumerator()) {
    $path = Join-Path $inputFolder $source.Key
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        curl.exe --fail --silent --show-error --location --retry 2 --max-time 120 $source.Value --output "$path.partial"
        if ($LASTEXITCODE -ne 0) { throw "Download failed for $($source.Key)" }
        Move-Item -LiteralPath "$path.partial" -Destination $path
    }
}

$sources.GetEnumerator() | ForEach-Object {
    $file = Get-FileHash -LiteralPath (Join-Path $inputFolder $_.Key)
    [pscustomobject]@{ File = $_.Key; Source = $_.Value; SHA256 = $file.Hash.ToLowerInvariant() }
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $inputFolder 'sources.json') -Encoding utf8
