/* ZenLink: DLL entry point for playfair (AirPlay FairPlay key decryption). */
#include "playfair.h"

__declspec(dllexport) void zl_playfair_decrypt(unsigned char *message3, unsigned char *cipher_text, unsigned char *key_out)
{
    playfair_decrypt(message3, cipher_text, key_out);
}
