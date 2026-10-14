Add-Type -AssemblyName System.Speech
$speak = New-Object System.Speech.Synthesis.SpeechSynthesizer

# List available voices
$voices = $speak.GetInstalledVoices()
Write-Host "Available voices:"
foreach ($v in $voices) {
    $vi = $v.VoiceInfo
    Write-Host ("  Name: " + $vi.Name + " | Culture: " + $vi.Culture.Name)
}

# Try Noémi, fallback to any Hungarian voice
$noemiVoice = $voices | Where-Object { $_.VoiceInfo.Name -like "*Noemi*" -or $_.VoiceInfo.Name -like "*Noémi*" }

if ($noemiVoice) {
    $speak.SelectVoice($noemiVoice.VoiceInfo.Name)
    Write-Host "Selected: $($noemiVoice.VoiceInfo.Name)"
} else {
    $huVoice = $voices | Where-Object { $_.VoiceInfo.Culture.Name -eq "hu-HU" } | Select-Object -First 1
    if ($huVoice) {
        $speak.SelectVoice($huVoice.VoiceInfo.Name)
        Write-Host "Selected: $($huVoice.VoiceInfo.Name)"
    } else {
        Write-Host "No Hungarian voice — using default"
    }
}

$speak.Rate = 0
$speak.Volume = 100

# Save to WAV file
$waveFile = "C:\Users\mater\.codync\bots\69ea5c5b-5783-44fd-9c74-505d948621ce\workspace\microscope-memory\tanulas_noemi.wav"
$text = Get-Content -Path "C:\Users\mater\.codync\bots\69ea5c5b-5783-44fd-9c74-505d948621ce\workspace\microscope-memory\speak_tanulas.txt" -Raw
$speak.SetOutputToWaveFile($waveFile)
$speak.Speak($text)
$speak.SetOutputToDefaultAudioDevice()
Write-Host "Saved: $waveFile"
Write-Host "Speaking..."
$speak.Speak($text)
Write-Host "Done."
