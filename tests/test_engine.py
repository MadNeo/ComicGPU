import io
import unittest
import zipfile
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw
from engine import read_archive, inspect_pages, restore, export_archive, split_page

class EngineTests(unittest.TestCase):
    def setUp(self):
        self.image = Image.new('RGB',(800,1000),'#f4e9ca')
        ImageDraw.Draw(self.image).rectangle((80,90,720,900),outline='black',width=5)

    def test_profiles_and_processing(self):
        for option in ['deskew','crop','grayscale','upscale']:
            result = restore(self.image,{option:True})
            self.assertGreater(result.width,0)
        for option in ['paper','shadow','bleed','moire']:
            self.assertEqual(restore(self.image,{option:.4}).size,self.image.size)

    def test_roundtrip_metadata_and_order(self):
        metadata = b'<ComicInfo><Title>Example</Title><PageCount>2</PageCount><Pages><Page Image="0"/><Page Image="1"/></Pages></ComicInfo>'
        data = export_archive([('10.png',self.image),('2.png',self.image)],metadata,[{},{}],set())
        pages,xml,warnings = read_archive(data,'test.cbz')
        self.assertEqual(len(pages),2)
        self.assertFalse(warnings)
        self.assertEqual(ET.fromstring(xml).findtext('Title'),'Example')
        self.assertTrue(inspect_pages(pages)[1]['review'])

    def test_split_and_exclusion_remap(self):
        spread = Image.new('RGB',(1600,800),'white')
        ImageDraw.Draw(spread).rectangle((800,0,1600,800),fill='black')
        self.assertEqual(split_page(spread,{'split':True,'rtl':True})[0].getpixel((10,10)),(0,0,0))
        metadata = b'<ComicInfo><PageCount>2</PageCount><Pages><Page Image="0"/><Page Image="1"/></Pages></ComicInfo>'
        output = export_archive([('a',self.image),('b',spread)],metadata,[{}, {'split':True}],{0})
        pages,xml,_ = read_archive(output,'x.cbz')
        self.assertEqual(len(pages),2)
        self.assertEqual([n.get('Image') for n in ET.fromstring(xml).find('Pages')],['0','1'])

    def test_corrupt_page_reported(self):
        buf=io.BytesIO()
        image=io.BytesIO(); self.image.save(image,'PNG')
        with zipfile.ZipFile(buf,'w') as archive:
            archive.writestr('10.png',image.getvalue())
            archive.writestr('2.png',image.getvalue())
            archive.writestr('3.png',b'broken')
        pages,_,warnings=read_archive(buf.getvalue(),'x.cbz')
        self.assertEqual(pages[0][0],'2.png')
        self.assertEqual(len(warnings),1)

    def test_empty_export_rejected(self):
        with self.assertRaises(ValueError):
            export_archive([('a',self.image)],None,[{}],{0})

if __name__ == '__main__':
    unittest.main()
